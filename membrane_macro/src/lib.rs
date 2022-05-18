extern crate proc_macro;
use membrane_types::c::CHeaderTypes;
use membrane_types::dart::{DartArgs, DartParams, DartTransforms};
use membrane_types::heck::MixedCase;
use membrane_types::rust::{RustArgs, RustExternParams, RustTransforms};
use membrane_types::{proc_macro2, quote, syn, Input, OutputStyle};
use options::{extract_options, Options};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::parse::{Parse, ParseStream, Result};
use syn::{parse_macro_input, AttributeArgs, Block, Expr, Ident, Path, Token, Type};

mod options;
mod parsers;

#[derive(Debug)]
struct ReprDart {
  fn_name: Ident,
  inputs: Vec<Input>,
  output_style: OutputStyle,
  output: Expr,
  error: Path,
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

    let callback_input = input.fork();
    syn::parenthesized!(arg_buffer in input);

    let (output_style, ret_type, err_type) = match input.fork().parse::<syn::ReturnType>()? {
      syn::ReturnType::Default => parsers::parse_type_from_callback(&callback_input)?,
      syn::ReturnType::Type(_, tp) => {
        input.parse::<Token![->]>()?;
        match *tp {
          syn::Type::ImplTrait(_) => parsers::parse_stream_return_type(input)?,
          _path => parsers::parse_return_type(input)?,
        }
      }
    };

    input.parse::<Block>()?;

    Ok(ReprDart {
      fn_name,
      inputs: parsers::parse_args(arg_buffer)?,
      output_style,
      output: ret_type,
      error: err_type,
    })
  }
}

#[proc_macro_attribute]
pub fn sync_dart(attrs: TokenStream, input: TokenStream) -> TokenStream {
  dart_impl(attrs, input, true)
}

#[proc_macro_attribute]
pub fn async_dart(attrs: TokenStream, input: TokenStream) -> TokenStream {
  dart_impl(attrs, input, false)
}

fn dart_impl(attrs: TokenStream, input: TokenStream, sync: bool) -> TokenStream {
  let Options {
    namespace,
    disable_logging,
    timeout,
    os_thread,
    callback,
  } = extract_options(
    parse_macro_input!(attrs as AttributeArgs),
    Options::default(),
    sync,
  );

  let mut functions = TokenStream::new();
  functions.extend(input.clone());

  let ReprDart {
    fn_name,
    output_style,
    output,
    error,
    mut inputs,
    ..
  } = parse_macro_input!(input as ReprDart);

  // we automatically provide the callback as the first argument to the user's
  // function and we enforce the type so here we drop the first parameter
  if callback && !inputs.is_empty() {
    inputs.remove(0);
  };

  let rust_outer_params: Vec<TokenStream2> = RustExternParams::from(&inputs).into();
  let rust_transforms: Vec<TokenStream2> = RustTransforms::from(&inputs).into();
  let rust_inner_args: Vec<Ident> = RustArgs::from(&inputs).into();

  let c_header_types: Vec<String> = CHeaderTypes::from(&inputs).into();

  let dart_outer_params: Vec<String> = DartParams::from(&inputs).into();
  let dart_transforms: Vec<String> = DartTransforms::from(&inputs).into();
  let dart_inner_args: Vec<String> = DartArgs::from(&inputs).into();

  let return_statement = match output_style {
    OutputStyle::EmitterSerialized => quote! {
      ::futures::executor::block_on(
        ::futures::future::Abortable::new(
          async move {
            let emitter = ::membrane::emitter::Handle::new(_port);
            let _: () = #fn_name(emitter, #(#rust_inner_args),*);
          }, membrane_future_registration)
        ).unwrap()
    },
    OutputStyle::EmitterStreamSerialized => quote! {
      ::futures::executor::block_on(
        ::futures::future::Abortable::new(
          async move {
            let emitter = ::membrane::emitter::StreamHandle::new(_port);
            let _: () = #fn_name(emitter, #(#rust_inner_args),*);
          }, membrane_future_registration)
        ).unwrap()
    },
    OutputStyle::StreamSerialized => quote! {
      crate::RUNTIME.spawn(
        ::futures::future::Abortable::new(
          async move {
            use ::membrane::futures::stream::StreamExt;
            let mut stream = #fn_name(#(#rust_inner_args),*);
            ::membrane::futures::pin_mut!(stream);
            let isolate = ::membrane::allo_isolate::Isolate::new(_port);
            while let Some(result) = stream.next().await {
              let result: ::std::result::Result<#output, #error> = result;
              ::membrane::utils::send::<#output, #error>(isolate, result);
            }
          }, membrane_future_registration)
        )
    },
    OutputStyle::Serialized if sync => quote! {
      let result: ::std::result::Result<#output, #error> = #fn_name(#(#rust_inner_args),*);
      let isolate = ::membrane::allo_isolate::Isolate::new(_port);
      ::membrane::utils::send::<#output, #error>(isolate, result);
    },
    OutputStyle::Serialized if os_thread => quote! {
      crate::RUNTIME.spawn_blocking(
        move || {
          ::futures::executor::block_on(
            ::futures::future::Abortable::new(
              async move {
                let result: ::std::result::Result<#output, #error> = #fn_name(#(#rust_inner_args),*).await;
                let isolate = ::membrane::allo_isolate::Isolate::new(_port);
                ::membrane::utils::send::<#output, #error>(isolate, result);
              }, membrane_future_registration)
          )
        }
      )
    },
    OutputStyle::Serialized => quote! {
      crate::RUNTIME.spawn(
        ::futures::future::Abortable::new(
          async move {
            let result: ::std::result::Result<#output, #error> = #fn_name(#(#rust_inner_args),*).await;
            let isolate = ::membrane::allo_isolate::Isolate::new(_port);
            ::membrane::utils::send::<#output, #error>(isolate, result);
          }, membrane_future_registration)
        )
    },
  };

  let extern_c_fn_name = Ident::new(
    format!("membrane_{}_{}", namespace, fn_name).as_str(),
    Span::call_site(),
  );

  let c_fn = quote! {
      #[no_mangle]
      #[allow(clippy::not_unsafe_ptr_arg_deref)]
      pub extern "C" fn #extern_c_fn_name(_port: i64, #(#rust_outer_params),*) -> *const ::membrane::TaskHandle {
          use ::membrane::{cstr, error, ffi_helpers};
          use ::std::ffi::CStr;

          let (membrane_future_handle, membrane_future_registration) = ::futures::future::AbortHandle::new_pair();

          #(#rust_transforms)*
          #return_statement;

          let handle = ::std::boxed::Box::new(::membrane::TaskHandle(membrane_future_handle));
          ::std::boxed::Box::into_raw(handle)
      }
  };

  functions.extend::<TokenStream>(c_fn.into());

  let c_name = extern_c_fn_name.to_string();
  let c_header_types = c_header_types.join(", ");
  let name = fn_name.to_string().to_mixed_case();
  let is_stream = [
    OutputStyle::StreamSerialized,
    OutputStyle::EmitterStreamSerialized,
  ]
  .contains(&output_style);
  let return_type = match &output {
    Expr::Tuple(_expr) => "()".to_string(),
    Expr::Path(expr) => expr.path.segments.last().unwrap().ident.to_string(),
    _ => unreachable!(),
  };
  let error_type = error.segments.last().unwrap().ident.to_string();
  let rust_arg_types = inputs
    .iter()
    .map(|Input { ty, .. }| ty)
    .collect::<Vec<&Type>>();

  let dart_outer_params = dart_outer_params.join(", ");
  let dart_transforms = dart_transforms.join(";\n    ");
  let dart_inner_args = dart_inner_args.join(", ");
  let timeout = if let Some(val) = timeout {
    quote! { Some(#val) }
  } else {
    quote! { None }
  };

  let _deferred_trace = quote! {
      ::membrane::inventory::submit! {
          #![crate = ::membrane]
          ::membrane::DeferredTrace {
              function: ::membrane::Function {
                extern_c_fn_name: #c_name.to_string(),
                extern_c_fn_types: #c_header_types.to_string(),
                fn_name: #name.to_string(),
                is_stream: #is_stream,
                return_type: #return_type.to_string(),
                error_type: #error_type.to_string(),
                namespace: #namespace.to_string(),
                disable_logging: #disable_logging,
                timeout: #timeout,
                dart_outer_params: #dart_outer_params.to_string(),
                dart_transforms: #dart_transforms.to_string(),
                dart_inner_args: #dart_inner_args.to_string(),
                output: "".to_string(),
              },
              namespace: #namespace.to_string(),
              trace: |
                tracer: &mut ::membrane::serde_reflection::Tracer,
                samples: &mut ::membrane::serde_reflection::Samples
              | {
                  tracer.trace_type::<#output>(samples).unwrap();
                  tracer.trace_type::<#error>(samples).unwrap();
                  // send all argument types over to serde-reflection, the primitives will be dropped
                  #(tracer.trace_type::<#rust_arg_types>(samples).unwrap();)*
              }
          }
      }
  };

  // by default only enable tracing in the dev profile or with an explicit flag
  #[cfg(all(
    any(debug_assertions, feature = "generate"),
    not(feature = "skip-generate")
  ))]
  functions.extend::<TokenStream>(_deferred_trace.into());

  functions
}

#[derive(Debug)]
struct ReprDartEnum {
  name: Ident,
}

impl Parse for ReprDartEnum {
  fn parse(input: ParseStream) -> Result<Self> {
    // parse and discard any other macros so that we can get to the enum
    let _ = input.call(syn::Attribute::parse_outer);
    let item_enum = input.parse::<syn::ItemEnum>()?;

    Ok(ReprDartEnum {
      name: item_enum.ident,
    })
  }
}

#[proc_macro_attribute]
pub fn dart_enum(attrs: TokenStream, input: TokenStream) -> TokenStream {
  let Options { namespace, .. } = extract_options(
    parse_macro_input!(attrs as AttributeArgs),
    Options::default(),
    false,
  );

  let mut variants = TokenStream::new();
  variants.extend(input.clone());

  let ReprDartEnum { name } = parse_macro_input!(input as ReprDartEnum);

  let _deferred_trace = quote! {
      ::membrane::inventory::submit! {
          #![crate = ::membrane]
          ::membrane::DeferredEnumTrace {
              namespace: #namespace.to_string(),
              trace: |
                tracer: &mut ::membrane::serde_reflection::Tracer
              | {
                  tracer.trace_simple_type::<#name>().unwrap();
              }
          }
      }
  };

  // by default only enable tracing in the dev profile or with an explicit flag
  #[cfg(all(
    any(debug_assertions, feature = "generate"),
    not(feature = "skip-generate")
  ))]
  variants.extend::<TokenStream>(_deferred_trace.into());

  variants
}
