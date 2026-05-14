use crate::options::{EnumOptions, FunctionOptions};
use crate::parsers;
use crate::utils;
use membrane_types::c::CHeaderTypes;
use membrane_types::dart::{DartArgs, DartParams, DartTransforms};
use membrane_types::heck::ToLowerCamelCase;
use membrane_types::rust::{flatten_types, RustArgs, RustExternParams, RustTransforms};
use membrane_types::{proc_macro2, quote, syn, Input, OutputStyle};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use std::convert::TryFrom;
use syn::{parse::Result, Ident, Type};

/// Parsed representation of a `#[async_dart]` / `#[sync_dart]` annotated function.
#[derive(Debug)]
pub(crate) struct ReprDart {
  pub(crate) fn_name: Ident,
  pub(crate) inputs: Vec<Input>,
  pub(crate) output_style: OutputStyle,
  pub(crate) output: syn::Type,
  pub(crate) error: syn::Type,
  pub(crate) docblock: String,
}

/// Generate the full token stream for a `#[async_dart]` / `#[sync_dart]` function.
pub(crate) fn to_token_stream(
  repr_dart: ReprDart,
  input: TokenStream,
  sync: bool,
  span: Span,
  options: FunctionOptions,
) -> Result<TokenStream> {
  let ReprDart {
    fn_name,
    output_style,
    output,
    error,
    inputs,
    docblock,
    ..
  } = repr_dart;

  let FunctionOptions {
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
    [
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
      // prepend the length of response into a single allocation, then box to shrink capacity
      let mut buffer = {
        let mut b = ::std::vec::Vec::with_capacity(8 + data.len());
        b.extend_from_slice(&len);
        b.extend(data);
        b.into_boxed_slice()
      };
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
    format!("membrane_{namespace}_{fn_name}").as_str(),
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

  #[allow(clippy::used_underscore_binding)]
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
                docblock: #docblock,
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

/// Generate the token stream for a `#[dart_enum]` annotated enum.
pub(crate) fn dart_enum_tokens(
  input: TokenStream,
  name: &Ident,
  options: EnumOptions,
) -> TokenStream {
  let EnumOptions {
    namespace, output, ..
  } = options;

  let mut variants = TokenStream::new();
  variants.extend(input);

  let enum_name = name.to_string();
  let output = if let Some(val) = output {
    quote! { Some(#val) }
  } else {
    quote! { None }
  };

  #[allow(clippy::used_underscore_binding)]
  let _deferred_trace = quote! {
      ::membrane::inventory::submit! {
          ::membrane::DeferredEnumTrace {
              enum_data: ::membrane::Enum {
                name: #enum_name,
                output: #output,
                namespace: #namespace
              },
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
