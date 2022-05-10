use crate::quote::quote;
use membrane_types::{syn, Input};
use syn::parse::{Parse, ParseBuffer, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::{Error, Expr, Ident, Path, Token};

pub fn parse_stream_return_type(input: ParseStream) -> Result<(Expr, Path)> {
  input.parse::<Token![impl]>()?;
  let span = input.span();
  let stream_ident = input.parse::<Ident>()?;
  input.parse::<Token![<]>()?;
  let item_ident = input.parse::<Ident>()?;

  if stream_ident != "Stream" || item_ident != "Item" {
    return Err(Error::new(span, "expected `impl Stream<Item = Result>`"));
  }

  input.parse::<Token![=]>()?;
  let (t, e) = parse_type(input)?;
  input.parse::<Token![>]>()?;

  Ok((t, e))
}

pub fn parse_return_type(input: ParseStream) -> Result<(Expr, Path)> {
  let (t, e) = parse_type(input)?;
  Ok((t, e))
}

pub fn parse_type_from_callback(input: ParseStream) -> Result<(Expr, Path)> {
  let buffer;
  syn::parenthesized!(buffer in input);

  buffer.parse::<Ident>()?;
  buffer.parse::<Token![:]>()?;
  buffer.parse::<Token![impl]>()?;
  let span = buffer.span();
  let name = buffer.parse::<Ident>()?;
  if !name.to_string().contains("Callback") {
    return Err(Error::new(
      span,
      "expected `impl membrane::Callback<Result<T, E>>`",
    ));
  }
  buffer.parse::<Token![<]>()?;
  Ok(parse_type(&buffer)?)
}

pub fn parse_type(input: ParseStream) -> Result<(Expr, Path)> {
  let outer_span = input.span();
  match input.parse::<Ident>()? {
    ident if ident == "Result" => (),
    _ => {
      return Err(Error::new(outer_span, "expected enum `Result`"));
    }
  }

  input.parse::<Token![<]>()?;

  let type_span = input.span();
  // handle the empty unit () type
  let t = if input.peek(syn::token::Paren) {
    let tuple = input.parse::<syn::ExprTuple>()?;
    if !tuple.elems.is_empty() {
      return Err(Error::new(
        type_span,
        "A tuple may not be returned from an `async_dart` function. If a tuple is needed return a struct containing the tuple.",
      ));
    }
    Expr::Tuple(tuple)
  } else {
    Expr::Path(input.parse::<syn::ExprPath>()?)
  };

  match input.parse::<Token![,]>() {
    Ok(_) => (),
    Err(_err) => {
      let type_name = match t {
        Expr::Path(syn::ExprPath { path, .. }) if !path.segments.is_empty() => {
          path.segments.first().unwrap().ident.to_string()
        }
        _ => String::new(),
      };

      match type_name.as_str() {
        "Vec" => {
          return Err(Error::new(type_span, "A vector may not be returned from an `async_dart` function. If a vector is needed return a struct containing the vector."));
        }
        _ => {
          return Err(Error::new(type_span, "expected a struct or scalar type"));
        }
      }
    }
  }

  let e = input.parse::<Path>()?;
  input.parse::<Token![>]>()?;

  Ok((t, e))
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
      _ => {
        panic!("self is not supported in #[async_dart] functions");
      }
    })
    .collect();

  Ok(inputs)
}
