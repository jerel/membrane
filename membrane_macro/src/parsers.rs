use crate::quote::quote;
use membrane_types::proc_macro2::Span;
use membrane_types::syn::spanned::Spanned;
use membrane_types::{syn, Input, OutputStyle};
use proc_macro::TokenStream;
use syn::parse::{Parse, ParseBuffer, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{Error, Expr, ExprPath, Ident, Token};

struct MaybeMutExpr(Expr);

impl Parse for MaybeMutExpr {
  fn parse(input: ParseStream) -> Result<Self> {
    let lookahead = input.lookahead1();
    if lookahead.peek(Token![mut]) {
      // if we find a `mut` token we just drop it as its not important to our type collection
      let _: Token![mut] = input.parse()?;
    }
    input.parse().map(MaybeMutExpr)
  }
}

pub fn parse_trait_return_type(input: ParseStream) -> Result<(OutputStyle, syn::Type, syn::Type)> {
  input.parse::<Token![impl]>()?;
  let span = input.span();
  let stream_path = input.parse::<ExprPath>()?;
  let stream_ident = &stream_path.path.segments.last().unwrap().ident;
  input.parse::<Token![<]>()?;

  match stream_ident.to_string().as_str() {
    "Future" => {
      let item_ident = input.parse::<Ident>()?;
      if item_ident != "Output" {
        return Err(Error::new(span, "expected `impl Future<Output = Result>`"));
      }

      input.parse::<Token![=]>()?;
      let (t, e) = parse_type(input)?;
      input.parse::<Token![>]>()?;
      Ok((OutputStyle::Serialized, t, e))
    }
    "Stream" => {
      let item_ident = input.parse::<Ident>()?;
      if item_ident != "Item" {
        return Err(Error::new(span, "expected `impl Stream<Item = Result>`"));
      }

      input.parse::<Token![=]>()?;
      let (t, e) = parse_type(input)?;
      input.parse::<Token![>]>()?;
      Ok((OutputStyle::StreamSerialized, t, e))
    }
    "StreamEmitter" => {
      let (t, e) = parse_type(input)?;
      input.parse::<Token![>]>()?;
      Ok((OutputStyle::StreamEmitterSerialized, t, e))
    }
    "Emitter" => {
      let (t, e) = parse_type(input)?;
      input.parse::<Token![>]>()?;
      Ok((OutputStyle::EmitterSerialized, t, e))
    }
    _ => {
      Err(Error::new(span, "trait found, expected `impl Stream<Item = Result>` or `impl StreamEmitter<Result>` or `impl Emitter<Result>`"))
    }
  }
}

pub fn parse_return_type(input: ParseStream) -> Result<(OutputStyle, syn::Type, syn::Type)> {
  let (t, e) = parse_type(input)?;
  Ok((OutputStyle::Serialized, t, e))
}

fn parse_type(input: ParseStream) -> Result<(syn::Type, syn::Type)> {
  let outer_span = input.span();
  let type_path = input.parse::<syn::TypePath>()?;
  let return_type = &type_path.path.segments.last().unwrap();
  if return_type.ident != "Result" {
    return Err(Error::new(outer_span, "expected enum `Result`"));
  }

  if let syn::PathArguments::AngleBracketed(args) = &return_type.arguments {
    let syn::AngleBracketedGenericArguments { args, .. } = args;
    return Ok((validate_type(&args[0])?, validate_type(&args[1])?));
  }

  Err(Error::new(outer_span, "expected enum `Result`"))
}

fn validate_type(type_: &syn::GenericArgument) -> Result<syn::Type> {
  if let syn::GenericArgument::Type(type_) = type_ {
    match type_ {
      syn::Type::Path(_path) => return Ok(type_.clone()),
      syn::Type::Tuple(tuple) if !tuple.elems.is_empty() => {
        return Err(Error::new(
        type_.span(),
        "A tuple may not be returned from an `async_dart` function. If a tuple is needed return a struct containing the tuple.",
      ));
      }
      // empty unit () is supported as a return type
      syn::Type::Tuple(_tuple) => return Ok(type_.clone()),
      _ => (),
    }
  }

  Err(Error::new(
    type_.span(),
    format!(
      "expected a struct, vec, or scalar type but found `{}`",
      quote! { #type_ }
    ),
  ))
}

pub(crate) fn parse_args(arg_buffer: ParseBuffer) -> Result<Vec<Input>> {
  let args: Punctuated<MaybeMutExpr, Token![,]> =
    arg_buffer.parse_terminated(MaybeMutExpr::parse)?;
  args
    .iter()
    .map(|arg| match arg {
      MaybeMutExpr(Expr::Type(syn::ExprType { ty, expr: var, .. })) => Ok(Input {
        variable: quote!(#var).to_string(),
        rust_type: quote!(#ty).to_string().split_whitespace().collect(),
        ty: *ty.clone(),
      }),
      MaybeMutExpr(ty) => Err(syn::Error::new_spanned(
        ty,
        "not a supported argument type for Dart interop",
      )),
    })
    .collect::<Result<Vec<_>>>()
}

pub(crate) fn add_port_to_args(input: TokenStream) -> TokenStream {
  let mut item = syn::parse_macro_input!(input as syn::ItemFn);
  let syn::Signature { inputs, .. } = item.sig;

  let mut args: syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma> =
    syn::punctuated::Punctuated::new();

  args.push(syn::FnArg::Typed(syn::PatType {
    attrs: vec![],
    pat: Box::new(syn::Pat::Ident(syn::PatIdent {
      attrs: vec![],
      by_ref: None,
      mutability: None,
      ident: Ident::new("_membrane_port", Span::call_site()),
      subpat: None,
    })),
    colon_token: syn::token::Colon(Span::call_site()),
    ty: Box::new(syn::Type::Path(syn::TypePath {
      qself: None,
      path: syn::Path::from(Ident::new("i64", Span::call_site())),
    })),
  }));

  args.extend(inputs);
  item.sig.inputs = args;

  quote!(#item).into()
}
