extern crate proc_macro;
use membrane_types::c::CHeaderTypes;
use membrane_types::dart::{DartArgs, DartParams, DartTransforms};
use membrane_types::heck::ToLowerCamelCase;
use membrane_types::rust::{flatten_types, RustArgs, RustExternParams, RustTransforms};
use membrane_types::{proc_macro2, quote, syn, Input, OutputStyle};
use options::{extract_options, Options};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use std::convert::TryFrom;
use syn::parse::{Parse, ParseStream, Result};
use syn::{parse_macro_input, punctuated::Punctuated, Block, Ident, MetaNameValue, Token, Type};

mod options;
mod parsers;
mod utils;

#[derive(Debug)]
struct ReprDart {
  fn_name: Ident,
  inputs: Vec<Input>,
  output_style: OutputStyle,
  output: syn::Type,
  error: syn::Type,
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
      parsers::parse_trait_return_type(input)?
    } else {
      parsers::parse_return_type(input)?
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

///
/// Apply this macro to Rust functions to mark them and their input/output types for Dart code generation.
///
/// Valid options:
///   * `namespace`, used to name the generated Dart API class and the implementation code directory.
///   * `disable_logging`, turn off logging statements inside generated Dart API code.
///   * `timeout`, the milliseconds that Dart should wait for a response on the isolate port before cancelling.
///   * `os_thread`, specifies that the function should be ran with `spawn_blocking` which moves the work to a pool of OS threads.
///
/// The usual function return type is either `Result<T, E>` or `impl Stream<Item = Result<T, E>>`. However, for
/// advanced usage you may want to use either `impl Emitter<Result<T, E>>` or `impl StreamEmitter<Result<T, E>>`.
/// When the Emitter traits are used the function must be synchronous but the emitter is thread-safe
/// and may be sent to another thread to send asynchronous messages. See the `example` directory for details.
///
#[proc_macro_attribute]
pub fn async_dart(attrs: TokenStream, input: TokenStream) -> TokenStream {
  dart_impl(attrs, input, false)
}

///
/// Apply this macro to synchronous Rust functions to mark them and their input/output types for Dart code generation.
///
/// WARNING: because it is synchronous a `#[sync_dart]` function will block the main Dart/Flutter thread until it
/// returns. Always use `#[async_dart]` unless you know you need synchronous behavior. For example, to interface with
/// a Rust/C library function that merely returns a pre-calculated value or tells the library to begin doing some
/// background work which will be reported later.
///
/// Valid options:
///   * `namespace`, used to name the generated Dart API class and the implementation code directory.
///   * `disable_logging`, turn off logging statements inside generated Dart API code.
///
/// The only supported function return type is `Result<T, E>`.
///
#[proc_macro_attribute]
pub fn sync_dart(attrs: TokenStream, input: TokenStream) -> TokenStream {
  dart_impl(attrs, input, true)
}

fn dart_impl(attrs: TokenStream, input: TokenStream, sync: bool) -> TokenStream {
  let options = match extract_options(
    parse_macro_input!(attrs with Punctuated::<MetaNameValue, Token![,]>::parse_terminated)
      .into_iter()
      .collect(),
    Options::default(),
    sync,
  ) {
    Ok(options) => options,
    Err(err) => {
      return syn::Error::new(Span::call_site(), err)
        .to_compile_error()
        .into();
    }
  };

  // get the most helpful span we can, either the function name if we can find it or else the beginning of that line
  let span: Span = if let Some(tree) = input
    .clone()
    .into_iter()
    .take_while(|x| matches!(x, proc_macro::TokenTree::Ident { .. }))
    .last()
  {
    tree.span().into()
  } else if let Some(tree) = input.clone().into_iter().next() {
    tree.span().into()
  } else {
    Span::call_site()
  };

  let input_two = input.clone();
  let repr_dart = parse_macro_input!(input as ReprDart);

  match to_token_stream(repr_dart, input_two, sync, span, options) {
    Ok(tokens) => tokens,
    Err(err) => err.to_compile_error().into(),
  }
}

fn to_token_stream(
  repr_dart: ReprDart,
  input: TokenStream,
  sync: bool,
  span: Span,
  options: Options,
) -> Result<TokenStream> {
  let ReprDart {
    fn_name,
    output_style,
    output,
    error,
    inputs,
    ..
  } = repr_dart;

  let Options {
    namespace,
    disable_logging,
    timeout,
    os_thread,
    borrow,
  } = options;

  let mut functions = TokenStream::new();

  match output_style {
    OutputStyle::StreamEmitterSerialized | OutputStyle::EmitterSerialized => {
      functions.extend(parsers::add_port_to_args(input))
    }
    _ => {
      functions.extend(input);
    }
  }

  let rust_fn_name = fn_name.to_string();
  let rust_outer_params: Vec<TokenStream2> = if sync {
    RustExternParams::try_from(&inputs)?.into()
  } else {
    vec![
      vec![quote! {membrane_port: i64}],
      RustExternParams::try_from(&inputs)?.into(),
    ]
    .concat()
  };
  let rust_transforms: Vec<TokenStream2> = RustTransforms::try_from(&inputs)?.into();
  let rust_inner_args: Vec<Ident> = RustArgs::from(&inputs).into();

  let c_header_types: Vec<String> = CHeaderTypes::try_from(&inputs)?.into();

  let dart_outer_params: Vec<String> = DartParams::try_from(&inputs)?.into();
  let dart_transforms: Vec<String> = DartTransforms::try_from(&inputs)?.into();
  let dart_inner_args: Vec<String> = DartArgs::from(&inputs).into();

  let return_statement = match output_style {
    OutputStyle::EmitterSerialized | OutputStyle::StreamEmitterSerialized if sync => {
      syn::Error::new(
        span,
        "#[sync_dart] expected a return type of `Result<T, E>` found an emitter",
      )
      .into_compile_error()
    }
    OutputStyle::EmitterSerialized | OutputStyle::StreamEmitterSerialized => quote! {
      let membrane_emitter = #fn_name(membrane_port, #(#rust_inner_args),*);
      let membrane_abort_handle = membrane_emitter.abort_handle();

      let handle = ::membrane::TaskHandle(::std::boxed::Box::new(membrane_abort_handle));
      ::std::boxed::Box::into_raw(Box::new(handle))
    },
    OutputStyle::StreamSerialized => quote! {
      let membrane_join_handle = crate::RUNTIME.get().info_spawn(
        async move {
          use ::membrane::futures::stream::StreamExt;
          let mut stream = #fn_name(#(#rust_inner_args),*);
          ::membrane::futures::pin_mut!(stream);
          let isolate = ::membrane::allo_isolate::Isolate::new(membrane_port);
          while let Some(result) = stream.next().await {
            let result: ::std::result::Result<#output, #error> = result;
            ::membrane::utils::send::<#output, #error>(isolate, result);
          }
        },
        ::membrane::runtime::Info { name: #rust_fn_name }
      );

      let handle = ::membrane::TaskHandle(::std::boxed::Box::new(move || { membrane_join_handle.abort() }));
      ::std::boxed::Box::into_raw(Box::new(handle))
    },
    OutputStyle::Serialized if sync => quote! {
      let result: ::std::result::Result<#output, #error> = #fn_name(#(#rust_inner_args),*);
      let ser_result = match result {
        Ok(value) => ::membrane::bincode::serialize(&(::membrane::MembraneMsgKind::Ok as u8, value)),
        Err(err) => ::membrane::bincode::serialize(&(::membrane::MembraneMsgKind::Error as u8, err)),
      };

      let data = if let Ok(data) = ser_result {
        data
      } else {
        vec![::membrane::MembraneMsgKind::Error as u8]
      };

      let len: [u8; 8] = (data.len() as i64).to_le_bytes();
      // prepend the length of response, then box the vec to shrink capacity
      let mut buffer = vec![len.to_vec(), data.clone()].concat().into_boxed_slice();
      let handle = buffer.as_mut_ptr();
      // forget so that Rust doesn't free while C is using it, we'll free it later
      ::std::mem::forget(buffer);
      handle
    },
    OutputStyle::Serialized if os_thread => quote! {
      let (membrane_future_handle, membrane_future_registration) = ::futures::future::AbortHandle::new_pair();

      crate::RUNTIME.get().info_spawn_blocking(
        move || {
          ::futures::executor::block_on(
            ::futures::future::Abortable::new(
              async move {
                let result: ::std::result::Result<#output, #error> = #fn_name(#(#rust_inner_args),*).await;
                let isolate = ::membrane::allo_isolate::Isolate::new(membrane_port);
                ::membrane::utils::send::<#output, #error>(isolate, result);
              }, membrane_future_registration)
          )
        },
        ::membrane::runtime::Info { name: #rust_fn_name }
      );

      let handle = ::membrane::TaskHandle(::std::boxed::Box::new(move || { membrane_future_handle.abort() }));
      ::std::boxed::Box::into_raw(Box::new(handle))
    },
    OutputStyle::Serialized => quote! {
      let membrane_join_handle = crate::RUNTIME.get().info_spawn(
        async move {
          let result: ::std::result::Result<#output, #error> = #fn_name(#(#rust_inner_args),*).await;
          let isolate = ::membrane::allo_isolate::Isolate::new(membrane_port);
          ::membrane::utils::send::<#output, #error>(isolate, result);
        },
        ::membrane::runtime::Info { name: #rust_fn_name }
      );

      let handle = ::membrane::TaskHandle(::std::boxed::Box::new(move || { membrane_join_handle.abort() }));
      ::std::boxed::Box::into_raw(Box::new(handle))
    },
  };

  let extern_c_fn_name = Ident::new(
    format!("membrane_{}_{}", namespace, fn_name).as_str(),
    Span::call_site(),
  );

  let c_fn = quote! {
      #[no_mangle]
      #[allow(clippy::not_unsafe_ptr_arg_deref)]
      pub extern "C" fn #extern_c_fn_name(#(#rust_outer_params),*) -> ::membrane::MembraneResponse {
        let func = || {
          use ::membrane::{cstr, error, ffi_helpers, runtime::Interface};
          use ::std::ffi::CStr;

          #(#rust_transforms)*
          #return_statement
        };

        let result = ::std::panic::catch_unwind(func)
          .map_err(|e| {
            ::membrane::ffi_helpers::panic::recover_panic_message(e)
              .unwrap_or_else(|| "The program panicked".to_string())
          });

        match result {
          Ok(ptr) => ::membrane::MembraneResponse{kind: ::membrane::MembraneResponseKind::Data, data: ptr as _},
          Err(error) => {
            let ptr = match ::std::ffi::CString::new(error) {
              Ok(c_string) => c_string,
              Err(error) => {
                // we don't expect this to ever happen
                ::std::ffi::CString::new(
                  format!("The program panicked and, additionally, panicked while reporting the error message. {}", error)).unwrap()
              }
            };
            ::membrane::MembraneResponse{kind: ::membrane::MembraneResponseKind::Panic, data: ptr.into_raw() as _}
          }
        }
      }
  };

  functions.extend::<TokenStream>(c_fn.into());

  let c_name = extern_c_fn_name.to_string();
  let c_header_types = c_header_types.join(", ");
  let dart_fn_name = rust_fn_name.to_lower_camel_case();
  let is_stream = [
    OutputStyle::StreamSerialized,
    OutputStyle::StreamEmitterSerialized,
  ]
  .contains(&output_style);

  let types = flatten_types(&output, vec![])?;
  let return_type = quote! { &[#(#types),*] };

  let types = flatten_types(&error, vec![])?;
  let error_type = quote! { &[#(#types),*] };

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
  let borrow = quote! { &[#(#borrow),*] };
  let debug_location = quote! { concat!(file!(), ":", line!()) };

  let _deferred_trace = quote! {
      ::membrane::inventory::submit! {
          ::membrane::DeferredTrace {
              function: ::membrane::Function {
                extern_c_fn_name: #c_name,
                extern_c_fn_types: #c_header_types,
                fn_name: #dart_fn_name,
                is_stream: #is_stream,
                is_sync: #sync,
                return_type: #return_type,
                error_type: #error_type,
                namespace: #namespace,
                disable_logging: #disable_logging,
                timeout: #timeout,
                borrow: #borrow,
                dart_outer_params: #dart_outer_params,
                dart_transforms: #dart_transforms,
                dart_inner_args: #dart_inner_args,
                output: "",
                location: #debug_location,
              },
              namespace: #namespace,
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

  functions = utils::maybe_inject_metadata(functions);

  Ok(functions)
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

///
/// Apply this macro to enums to mark them for Dart code generation.
///
/// Valid options:
///   * `namespace`, used to select the Dart implementation code directory.
#[proc_macro_attribute]
pub fn dart_enum(attrs: TokenStream, input: TokenStream) -> TokenStream {
  let Options {
    namespace, borrow, ..
  } = match extract_options(
    parse_macro_input!(attrs with Punctuated::<MetaNameValue, Token![,]>::parse_terminated)
      .into_iter()
      .collect(),
    Options::default(),
    false,
  ) {
    Ok(options) => options,
    Err(err) => {
      return syn::Error::new(Span::call_site(), err)
        .to_compile_error()
        .into();
    }
  };

  let mut variants = TokenStream::new();
  variants.extend(input.clone());

  if !borrow.is_empty() {
    variants.extend::<TokenStream>(
      syn::Error::new(
        Span::call_site(),
        "`borrow` is not a valid option for #[dart_enum]",
      )
      .to_compile_error()
      .into(),
    );
  }

  let ReprDartEnum { name } = parse_macro_input!(input as ReprDartEnum);
  let enum_name = name.to_string();

  let _deferred_trace = quote! {
      ::membrane::inventory::submit! {
          ::membrane::DeferredEnumTrace {
              name: #enum_name,
              namespace: #namespace,
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

///
/// For use inside `#[async_dart]` functions. Used to create an emitter for use with `impl Emitter<Result<T, E>>` and `impl StreamEmitter<Result<T, E>>`
/// return types.
///
/// Example:
///
/// ```ignore
/// #[async_dart(namespace = "example")]
/// pub fn some_function() -> impl StreamEmitter<Result<i32, String>> {
///   let stream = emitter!();
///
///   let s = stream.clone();
///   thread::spawn(move || {
///     s.push(Ok(1));
///     s.push(Ok(2));
///   });
///
///   stream
/// }
/// ```
#[proc_macro]
pub fn emitter(_item: TokenStream) -> TokenStream {
  "{
    use ::membrane::emitter::Emitter;
    ::membrane::emitter::Handle::new(_membrane_port)
  }"
  .parse()
  .unwrap()
}

///
/// A helper macro that can be used to ensure that Membrane types are still accessible
/// if the workspace crate which generates the `cdylib` binary has no instances
/// of Membrane macros such as `#[async_dart]` or `#[sync_dart]`.
///
/// Example:
///
/// // crate_one/src/lib.rs
/// #[async_dart(namespace = "one")]
/// pub fn example()
///
/// // crate_two/Cargo.toml
/// [lib]
/// crate-type = ["cdylib"]
///
/// // crate_two/src/lib.rs
/// use crate_one::*;
/// membrane::export_metadata!();
///
#[proc_macro]
pub fn export_metadata(token_stream: TokenStream) -> TokenStream {
  if !utils::is_cdylib() {
    syn::Error::new(
      Span::call_site(),
      "membrane::export_metadata!() was used in a crate which is not `crate-type` of `cdylib`.
      Either it is being invoked in a crate which exports code instead of generating a cdylib
      (and is consequently unnecessary and should be removed) or it is being used in the crate which is responsible
      for generating the cdylib but the `crate-type` was accidentally omitted from `[lib]` in `Cargo.toml`.",
    )
    .to_compile_error()
    .into()
  } else {
    utils::maybe_inject_metadata(token_stream)
  }
}
