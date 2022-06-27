use membrane_types::syn;
use syn::{Lit, Meta, MetaNameValue, NestedMeta};

#[derive(Debug, Default)]
pub(crate) struct Options {
  pub namespace: String,
  pub disable_logging: bool,
  pub timeout: Option<i32>,
  pub os_thread: bool,
}

pub(crate) fn extract_options(
  mut input: Vec<NestedMeta>,
  mut options: Options,
  sync: bool,
) -> Options {
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
    Some((ident, Lit::Bool(val))) if ident == "disable_logging" => {
      options.disable_logging = val.value();
      options
    }
    Some((ident, Lit::Int(val))) if ident == "timeout" && !sync => {
      options.timeout = Some(val.base10_parse().unwrap());
      options
    }
    Some((ident, _)) if ident == "os_thread" && sync => {
      invalid_option("sync_dart", "os_thread=true")
    }
    Some((ident, Lit::Bool(val))) if ident == "os_thread" => {
      options.os_thread = val.value();
      options
    }
    Some(_) if sync => {
      panic!(r#"#[sync_dart] only `namespace=""` and `disable_logging=true` are valid options"#);
    }
    Some(_) => {
      panic!(
        r#"#[async_dart] only `namespace=""`, `disable_logging=true`, `os_thread=true`, and `timeout=1000` are valid options"#
      );
    }
    None => {
      // we've iterated over all options and didn't find a namespace (required)
      if options.namespace.is_empty() {
        panic!(
          "#[{}] expects a `namespace` attribute",
          if sync { "sync_dart" } else { "async_dart" }
        );
      }

      return options;
    }
  };

  extract_options(input, options, sync)
}

fn invalid_option(macr: &str, opt: &str) -> ! {
  panic!(
    "#[{m}] `{opt}` is not a valid option for `{m}`",
    m = macr,
    opt = opt
  );
}
