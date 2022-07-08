use crate::quote::quote;
use membrane_types::proc_macro2::Span;
use membrane_types::{syn, Input, OutputStyle};
use proc_macro::TokenStream;
use syn::parse::{Parse, ParseBuffer, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{Error, Expr, ExprPath, Ident, Token};

pub fn parse_trait_return_type(input: ParseStream) -> Result<(OutputStyle, syn::Type, syn::Type)> {
  input.parse::<Token![impl]>()?;
  let span = input.span();
  let stream_path = input.parse::<ExprPath>()?;
  let stream_ident = &stream_path.path.segments.last().unwrap().ident;
  input.parse::<Token![<]>()?;

  match stream_ident.to_string().as_str() {
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

  match &return_type.arguments {
    syn::PathArguments::AngleBracketed(args) => match args {
      syn::AngleBracketedGenericArguments { args, .. } => {
        Ok((validate_type(&args[0])?, validate_type(&args[1])?))
      }
    },
    _ => Err(Error::new(outer_span, "expected enum `Result`")),
  }
}

fn validate_type(type_: &syn::GenericArgument) -> Result<syn::Type> {
  match type_ {
    syn::GenericArgument::Type(type_) => match type_ {
      syn::Type::Path(_path) => return Ok(type_.clone()),
      syn::Type::Tuple(tuple) if tuple.elems.len() > 0 => {
        return Err(Error::new(
        Span::call_site(),
        "A tuple may not be returned from an `async_dart` function. If a tuple is needed return a struct containing the tuple.",
      ));
      }
      // empty unit () is supported as a return type
      syn::Type::Tuple(_tuple) => return Ok(type_.clone()),
      _ => (),
    },
    _ => (),
  };

  Err(Error::new(
    Span::call_site(),
    format!(
      "expected a struct, vec, or scalar type but found {:?}",
      type_
    ),
  ))
}

pub(crate) fn parse_args(arg_buffer: ParseBuffer) -> Result<Vec<Input>> {
  let args: Punctuated<Expr, Token![,]> = arg_buffer.parse_terminated(Expr::parse)?;
  let inputs = args
    .iter()
    .map(|arg| match arg {
      Expr::Type(syn::ExprType { ty, expr: var, .. }) => Input {
        variable: quote!(#var).to_string(),
        rust_type: quote!(#ty).to_string().split_whitespace().collect(),
        ty: *ty.clone(),
      },
      Expr::Binary(syn::ExprBinary {
        left, right, op, ..
      }) if op == &syn::BinOp::Add(syn::token::Add(arg_buffer.span())) => {
        handle_binop_add(left, right)
      }
      _ => {
        panic!("self is not supported in #[async_dart] functions");
      }
    })
    .collect();

  Ok(inputs)
}

fn handle_binop_add(left: &Expr, right: &Expr) -> Input {
  match (left, right) {
    (Expr::Type(syn::ExprType { ty, expr: var, .. }), Expr::Path(syn::ExprPath { path, .. }))
      if path.segments.last().is_some() && path.segments.last().unwrap().ident == "Clone" =>
    {
      Input {
        variable: quote!(#var).to_string(),
        rust_type: quote!(#ty).to_string().split_whitespace().collect(),
        ty: *ty.clone(),
      }
    }
    _ => {
      panic!("the only constraint supported in #[async_dart] function args is `+ Clone`")
    }
  }
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
