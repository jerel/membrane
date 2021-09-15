extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::fmt;
use syn::parse::{Parse, ParseStream, Result};
use syn::{parse_macro_input, Block, Error, FnArg, Ident, LitStr, PatType, Path, Token};

#[derive(Debug)]
struct Input {
  variable: String,
  rust_type: String,
  ty: syn::Type,
}

#[derive(Debug, PartialEq, Eq)]
enum OutputStyle {
  StreamSerialized,
  Serialized,
}

impl fmt::Display for OutputStyle {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

struct ReprDartAttrs {
  namespace: String,
}

impl Parse for ReprDartAttrs {
  fn parse(input: ParseStream) -> Result<Self> {
    let name_token: Ident = input.parse()?;
    if name_token != "namespace" {
      return Err(Error::new(
        name_token.span(),
        "#[async_dart] expects a `namespace` attribute",
      ));
    }
    input.parse::<Token![=]>()?;
    let s: LitStr = input.parse()?;
    Ok(ReprDartAttrs {
      namespace: s.value(),
    })
  }
}

#[derive(Debug)]
struct ReprDart {
  fn_name: Ident,
  inputs: Vec<Input>,
  output_style: OutputStyle,
  output: Path,
  error: Path,
  block: Block,
}

impl Parse for ReprDart {
  fn parse(input: ParseStream) -> Result<Self> {
    let arg_buffer;

    input.parse::<Token![pub]>()?;
    if input.peek(Token![async]) {
      input.parse::<Token![async]>()?;
    }
    input.parse::<Token![fn]>()?;
    let fn_name = input.parse::<Ident>()?;
    syn::parenthesized!(arg_buffer in input);
    input.parse::<Token![->]>()?;
    let (output_style, ret_type, err_type) = if input.peek(Token![impl]) {
      input.parse::<Token![impl]>()?;
      input.parse::<Ident>()?;
      input.parse::<Token![<]>()?;
      input.parse::<Ident>()?;
      input.parse::<Token![=]>()?;
      input.parse::<Ident>()?;
      input.parse::<Token![<]>()?;
      let t = input.parse::<Path>()?;
      input.parse::<Token![,]>()?;
      let e = input.parse::<Path>()?;
      input.parse::<Token![>]>()?;
      input.parse::<Token![>]>()?;
      (OutputStyle::StreamSerialized, t, e)
    } else {
      input.parse::<Ident>()?;
      input.parse::<Token![<]>()?;
      let t = input.parse::<Path>()?;
      input.parse::<Token![,]>()?;
      let e = input.parse::<Path>()?;
      input.parse::<Token![>]>()?;
      (OutputStyle::Serialized, t, e)
    };
    let block = input.parse::<Block>()?;

    Ok(ReprDart {
      fn_name,
      inputs: {
        let mut args = Vec::new();
        while !arg_buffer.is_empty() {
          match arg_buffer.parse()? {
            FnArg::Typed(PatType { pat, ty, .. }) => {
              args.push(Input {
                variable: quote!(#pat).to_string(),
                rust_type: quote!(#ty).to_string(),
                ty: *ty,
              });
            }
            FnArg::Receiver(_) => {
              panic!("self is not supported in #[async_dart] functions");
            }
          }
        }
        args
      },
      output_style,
      output: ret_type,
      error: err_type,
      block,
    })
  }
}

fn to_c_type(ty: &str) -> proc_macro2::TokenStream {
  match ty {
    "String" => quote!(*const ::std::os::raw::c_char),
    _ => panic!("c type not yet supported"),
  }
}

fn cast_c_type(ty: &str, variable: &str) -> proc_macro2::TokenStream {
  match ty {
    "String" => {
      let variable = Ident::new(variable, Span::call_site());
      quote!(cstr!(#variable).to_string())
    }
    _ => panic!("casting c type not yet supported"),
  }
}

#[proc_macro_attribute]
pub fn async_dart(attrs: TokenStream, input: TokenStream) -> TokenStream {
  let ReprDartAttrs { namespace } = parse_macro_input!(attrs as ReprDartAttrs);

  let mut functions = TokenStream::new();
  functions.extend(input.clone());

  let ReprDart {
    fn_name,
    output_style,
    output,
    error,
    inputs,
    ..
  } = parse_macro_input!(input as ReprDart);

  let outer_rust_inputs: Vec<proc_macro2::TokenStream> = inputs
    .iter()
    .map(|i| {
      let variable = Ident::new(&i.variable, Span::call_site());
      let c_type = to_c_type(&i.rust_type);
      quote!(#variable: #c_type)
    })
    .collect();

  let transform_rust_inputs: Vec<proc_macro2::TokenStream> = inputs
    .iter()
    .map(|i| {
      let variable = Ident::new(&i.variable, Span::call_site());
      let cast = cast_c_type(&i.rust_type, &i.variable);

      quote!(let #variable = #cast;)
    })
    .collect();

  let inner_rust_inputs: Vec<Ident> = inputs
    .iter()
    .map(|i| Ident::new(&i.variable, Span::call_site()))
    .collect();

  let serializer = quote! {
      match result {
          Ok(value) => {
              if let Ok(buffer) = ::membrane::bincode::serialize(&(true, value)) {
                  isolate.post(::membrane::allo_isolate::ZeroCopyBuffer(buffer));
              }
          }
          Err(err) => {
              if let Ok(buffer) = ::membrane::bincode::serialize(&(false, err)) {
                  isolate.post(::membrane::allo_isolate::ZeroCopyBuffer(buffer));
              }
          }
      };
  };

  let return_statement = match output_style {
    OutputStyle::StreamSerialized => {
      quote! {
          use ::futures::stream::StreamExt;
          let mut stream = #fn_name(#(#inner_rust_inputs),*);
          while let Some(result) = stream.next().await {
              let result: ::std::result::Result<#output, #error> = result;
              #serializer
          }
      }
    }
    OutputStyle::Serialized => quote! {
        let result: ::std::result::Result<#output, #error> = #fn_name(#(#inner_rust_inputs),*).await;
        #serializer
    },
  };

  let c_fn_name = Ident::new(
    format!("membrane_{}_{}", namespace, fn_name).as_str(),
    Span::call_site(),
  );

  let c_fn = quote! {
      #[no_mangle]
      #[allow(clippy::not_unsafe_ptr_arg_deref)]
      pub extern "C" fn #c_fn_name(port: i64, #(#outer_rust_inputs),*) -> i32 {
          use crate::RUNTIME;
          use ::membrane::{cstr, error, ffi_helpers};
          use ::std::ffi::CStr;

          let isolate = ::membrane::allo_isolate::Isolate::new(port);

          #(#transform_rust_inputs)*
          RUNTIME.spawn(async move {
              #return_statement
          });

          1
      }
  };

  functions.extend::<TokenStream>(c_fn.into());

  let c_name = c_fn_name.to_string();
  let name = fn_name.to_string();
  let is_stream = output_style == OutputStyle::StreamSerialized;
  let return_type = output.segments.last().unwrap().ident.to_string();
  let error_type = error.segments.last().unwrap().ident.to_string();

  let _deferred_trace = quote! {
      ::membrane::inventory::submit! {
          #![crate = ::membrane]
          ::membrane::DeferredTrace {
              function: ::membrane::Function {
                c_fn_name: #c_name.to_string(),
                fn_name: #name.to_string(),
                c_fn_args: "".to_string(),
                fn_args: "".to_string(),
                is_stream: #is_stream,
                return_type: #return_type.to_string(),
                error_type: #error_type.to_string(),
                namespace: #namespace.to_string(),
                output: "".to_string(),
              },
              namespace: #namespace.to_string(),
              trace: |tracer: &mut ::membrane::serde_reflection::Tracer| {
                  tracer.trace_type::<#output>(&::membrane::serde_reflection::Samples::new()).unwrap();
                  tracer.trace_type::<#error>(&::membrane::serde_reflection::Samples::new()).unwrap();
                  tracer
              }
          }
      }
  };

  #[cfg(feature = "generate")]
  functions.extend::<TokenStream>(_deferred_trace.into());

  functions
}
