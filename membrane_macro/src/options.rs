use membrane_types::syn;
use syn::{Lit, Meta, MetaNameValue, NestedMeta};

#[derive(Debug, Default)]
pub(crate) struct Options {
  pub namespace: String,
  pub disable_logging: bool,
  pub timeout: Option<i32>,
  pub os_thread: bool,
  pub borrow: Vec<String>,
}

pub(crate) fn extract_options(
  mut input: Vec<NestedMeta>,
  mut options: Options,
  sync: bool,
) -> Result<Options, String> {
  let option = match input.pop() {
    Some(NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit, .. }))) => {
      let ident = path.get_ident().unwrap().clone();
      Some((ident, lit))
    }
    _ => None,
  };

  let options = match option {
    Some((ident, Lit::Str(val))) if ident == "namespace" => {
      options.namespace = val.value();
      options
    }
    Some((ident, Lit::Str(val))) if ident == "borrow" => {
      let value = val.value();
      let borrow = value.split("::").collect::<Vec<&str>>();
      // check the given borrow for correctness, then push the string
      if let [_namespace, _type] = borrow[..] {
        options.borrow.push(value);
        options
      } else {
        return Err(format!(
          "`{value}` is not a valid option for `borrow`, must be of the form `borrow = \"namespace::Type\"`",
          value = value
        ));
      }
    }
    Some((ident, Lit::Bool(val))) if ident == "disable_logging" => {
      options.disable_logging = val.value();
      options
    }
    Some((ident, Lit::Int(val))) if ident == "timeout" && !sync => {
      options.timeout = Some(val.base10_parse().unwrap());
      options
    }
    Some((ident, Lit::Bool(val))) if ident == "timeout" && !sync => {
      if val.value == true {
        return Err(
          "`true` is not a valid option for `timeout`, must be an integer or `false`".to_string(),
        );
      }
      options.timeout = Some(-1);
      options
    }
    Some((ident, _)) if ident == "os_thread" && sync => {
      return invalid_option("sync_dart", "os_thread=true");
    }
    Some((ident, Lit::Bool(val))) if ident == "os_thread" => {
      options.os_thread = val.value();
      options
    }
    Some(_) if sync => {
      return Err(
        r#"only `namespace=""`, `borrow="namespace::Type"`, and `disable_logging=true` are valid options"#.to_string(),
      );
    }
    Some(_) => {
      return Err(
        r#"only `namespace=""`, `borrow="namespace::Type"`, `disable_logging=true`, `os_thread=true`, and `timeout=1000` are valid options"#.to_string());
    }
    None => {
      // we've iterated over all options and didn't find a namespace (required)
      if options.namespace.is_empty() {
        return Err(format!(
          "#[{}] expects a `namespace` attribute",
          if sync { "sync_dart" } else { "async_dart" }
        ));
      }

      return Ok(options);
    }
  };

  extract_options(input, options, sync)
}

fn invalid_option(macr: &str, opt: &str) -> Result<Options, String> {
  Err(format!(
    "`{opt}` is not a valid option for `{m}`",
    m = macr,
    opt = opt
  ))
}
