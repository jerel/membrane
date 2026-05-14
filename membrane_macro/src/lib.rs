extern crate proc_macro;
use membrane_types::syn::Attribute;
use membrane_types::{proc_macro2, quote, syn};
use options::{extract_enum_options, extract_function_options, FunctionOptions};
use proc_macro::TokenStream;
use proc_macro2::Span;
use syn::parse::{Parse, ParseStream, Result};
use syn::{parse_macro_input, punctuated::Punctuated, Block, Ident, MetaNameValue, Token};

use crate::codegen::ReprDart;
use crate::options::EnumOptions;

mod codegen;
mod options;
mod parsers;
mod utils;

impl Parse for ReprDart {
  fn parse(input: ParseStream) -> Result<Self> {
    let arg_buffer;

    let docblock = input
      .call(Attribute::parse_outer)?
      .iter()
      .map(|x| x.meta.require_name_value())
      .map(|x: Result<&MetaNameValue>| Ok(x?.value.clone()))
      .filter_map(|x: Result<syn::Expr>| match x {
        Ok(syn::Expr::Lit(syn::ExprLit {
          lit: syn::Lit::Str(comment),
          ..
        })) => Some(Ok(format!("/// {}\n", &comment.value()))),
        _ => None,
      })
      .collect::<Result<Vec<String>>>()?
      .join("");

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
      inputs: parsers::parse_args(&arg_buffer)?,
      output_style,
      output: ret_type,
      error: err_type,
      docblock,
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
  let options = match extract_function_options(
    parse_macro_input!(attrs with Punctuated::<MetaNameValue, Token![,]>::parse_terminated)
      .into_iter()
      .collect(),
    FunctionOptions::default(),
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

  match codegen::to_token_stream(repr_dart, input_two, sync, span, options) {
    Ok(tokens) => tokens,
    Err(err) => err.to_compile_error().into(),
  }
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
///   * `output`, used to override global enum config of `enum`, `sealed`, or `abstract`.
#[proc_macro_attribute]
pub fn dart_enum(attrs: TokenStream, input: TokenStream) -> TokenStream {
  let options = match extract_enum_options(
    parse_macro_input!(attrs with Punctuated::<MetaNameValue, Token![,]>::parse_terminated)
      .into_iter()
      .collect(),
    EnumOptions::default(),
  ) {
    Ok(options) => options,
    Err(err) => {
      return syn::Error::new(Span::call_site(), err)
        .to_compile_error()
        .into();
    }
  };

  let input_two = input.clone();
  let ReprDartEnum { name } = parse_macro_input!(input as ReprDartEnum);

  codegen::dart_enum_tokens(input_two, &name, options)
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
/// `crate-type` = \["cdylib"\]
///
/// // crate_two/src/lib.rs
/// use crate_one::*;
/// membrane::export_metadata!();
///
#[proc_macro]
pub fn export_metadata(token_stream: TokenStream) -> TokenStream {
  if utils::is_cdylib() {
    utils::maybe_inject_metadata(token_stream)
  } else {
    syn::Error::new(
      Span::call_site(),
      "membrane::export_metadata!() was used in a crate which is not `crate-type` of `cdylib`.
      Either it is being invoked in a crate which exports code instead of generating a cdylib
      (and is consequently unnecessary and should be removed) or it is being used in the crate which is responsible
      for generating the cdylib but the `crate-type` was accidentally omitted from `[lib]` in `Cargo.toml`.",
    )
    .to_compile_error()
    .into()
  }
}
