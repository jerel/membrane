use crate::{config::DartConfig, metadata, utils, Membrane};
use serde_reflection::{Registry, Samples, Tracer, TracerConfig};
use std::{
  collections::{BTreeMap, BTreeSet, HashMap},
  path::PathBuf,
  process::exit,
};
use tracing::info;

pub(crate) type Namespace = &'static str;
pub(crate) type Borrows =
  HashMap<Namespace, BTreeMap<&'static str, (BTreeSet<&'static str>, ExplicitBorrowLocations)>>;
pub(crate) type SourceCodeLocation = &'static str;
pub(crate) type ExplicitBorrowLocations = HashMap<&'static str, Vec<SourceCodeLocation>>;

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct Function {
  pub extern_c_fn_name: &'static str,
  pub extern_c_fn_types: &'static str,
  pub fn_name: &'static str,
  pub is_stream: bool,
  pub is_sync: bool,
  pub return_type: &'static [&'static str],
  pub error_type: &'static [&'static str],
  pub namespace: &'static str,
  pub disable_logging: bool,
  pub timeout: Option<i32>,
  pub borrow: &'static [&'static str],
  pub output: &'static str,
  pub dart_outer_params: &'static str,
  pub dart_transforms: &'static str,
  pub dart_inner_args: &'static str,
  pub location: SourceCodeLocation,
  pub docblock: &'static str,
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct Enum {
  pub name: &'static str,
  pub output: Option<&'static str>,
  pub namespace: &'static str,
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct DeferredTrace {
  pub function: Function,
  pub namespace: &'static str,
  pub trace: fn(tracer: &mut serde_reflection::Tracer, samples: &mut serde_reflection::Samples),
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct DeferredEnumTrace {
  pub enum_data: Enum,
  pub namespace: &'static str,
  pub trace: fn(tracer: &mut serde_reflection::Tracer),
}

inventory::collect!(DeferredTrace);
inventory::collect!(DeferredEnumTrace);

/// Output of the metadata loading and serde-reflection tracing pipeline.
pub(crate) struct RegistryData {
  pub(crate) errors: Vec<String>,
  pub(crate) namespaces: Vec<&'static str>,
  pub(crate) namespaced_registry: HashMap<&'static str, serde_reflection::Result<Registry>>,
  pub(crate) namespaced_fn_registry: HashMap<&'static str, Vec<Function>>,
  pub(crate) namespaced_enum_registry: HashMap<&'static str, Vec<Enum>>,
  pub(crate) borrows: Borrows,
  pub(crate) input_libs: Vec<libloading::Library>,
}

/// Load metadata from inventory (local source) or a cdylib, then trace all
/// types through serde-reflection to build per-namespace registries.
pub(crate) fn build<P>(cdylib_path: Option<&P>) -> RegistryData
where
  P: ?Sized + std::convert::AsRef<std::path::Path> + std::fmt::Debug,
{
  let mut errors = vec![];
  let mut input_libs = vec![];

  let (mut enums, mut functions) = match cdylib_path {
    None => {
      info!("No `lib.so` paths were passed, generating code from local `lib` source");
      (metadata::enums(), metadata::functions())
    }
    Some(lib_path) => {
      let (enums, functions, version, _membrane_version) =
        match metadata::extract_metadata_from_cdylib(lib_path.as_ref().as_os_str(), &mut input_libs)
        {
          Ok(symbols) => symbols,
          Err(msg) => {
            errors.push(msg);
            panic!();
          }
        };

      info!(
        "Generating code from {:?} which was compiled at version {:?}",
        lib_path, version
      );
      (enums, functions)
    }
  };

  if enums.is_empty() && functions.is_empty() {
    info!(
      "No type information could be found. Do you have #[async_dart] or #[sync_dart] in your code?"
    );
  }

  enums.sort_by_cached_key(|e| &e.enum_data.name);

  functions.sort_by_cached_key(|f| {
    format!(
      "{}{}{}",
      f.function.is_stream, f.function.is_sync, f.function.fn_name
    )
  });

  let mut namespaces = [
    enums.iter().map(|x| x.namespace).collect::<Vec<&str>>(),
    functions.iter().map(|x| x.namespace).collect::<Vec<&str>>(),
  ]
  .concat();

  namespaces.sort_unstable();
  namespaces.dedup();

  let mut namespaced_registry: HashMap<&str, Tracer> = HashMap::new();
  let mut namespaced_samples = HashMap::new();
  let mut namespaced_fn_registry = HashMap::new();
  let mut namespaced_enum_registry = HashMap::new();
  let mut borrows: Borrows = HashMap::new();

  // collect all the metadata about functions (without tracing them yet)
  for item in &functions {
    namespaced_fn_registry
      .entry(item.namespace)
      .or_insert_with(Vec::new)
      .push(item.function.clone());
  }

  // work out which namespaces borrow which types from other namespaces
  for namespace in &namespaces {
    create_borrows(&namespaced_fn_registry, namespace, &mut borrows);
  }

  // collect all the metadata about enums (without tracing them yet)
  for item in &enums {
    namespaced_enum_registry
      .entry(item.namespace)
      .or_insert_with(Vec::new)
      .push(item.enum_data.clone());
  }

  // trace all the enums at least once
  for item in &enums {
    // trace the enum into the borrowing namespace's registry
    for (for_namespace, from_namespaces) in &borrows {
      if let Some((types, _location)) = from_namespaces.get(item.namespace) {
        if types.contains(item.enum_data.name) {
          let tracer = namespaced_registry
            .entry(*for_namespace)
            .or_insert_with(|| Tracer::new(TracerConfig::default()));

          (item.trace)(tracer);
        }
      }
    }

    // trace the enum into the owning namespace's registry
    let tracer = namespaced_registry
      .entry(item.namespace)
      .or_insert_with(|| Tracer::new(TracerConfig::default()));

    (item.trace)(tracer);
  }

  // now that we have the enums in the registry we'll trace each of the functions
  for item in &functions {
    let tracer = namespaced_registry
      .entry(item.namespace)
      .or_insert_with(|| Tracer::new(TracerConfig::default()));

    let samples = namespaced_samples
      .entry(item.namespace)
      .or_insert_with(Samples::new);

    (item.trace)(tracer, samples);
  }

  RegistryData {
    errors,
    namespaces,
    namespaced_registry: namespaced_registry
      .into_iter()
      .map(|(key, val)| (key, val.registry()))
      .collect(),
    namespaced_fn_registry,
    namespaced_enum_registry,
    borrows,
    input_libs,
  }
}

/// Resolve which types each namespace borrows from other namespaces.
pub(crate) fn create_borrows(
  namespaced_fn_registry: &HashMap<&str, Vec<Function>>,
  namespace: &'static str,
  borrows: &mut Borrows,
) {
  let default = &vec![];
  let fns = namespaced_fn_registry.get(namespace).unwrap_or(default);

  for fun in fns {
    for borrow_list in fun
      .borrow
      .iter()
      .map(|borrow| borrow.split("::").map(|x| x.trim()).collect::<Vec<&str>>())
    {
      if let [from_namespace, r#type] = borrow_list[..] {
        let imports = borrows.entry(namespace).or_default();
        let (types, source_code_locations) = imports
          .entry(from_namespace)
          .or_insert((BTreeSet::new(), HashMap::new()));
        types.insert(r#type);
        source_code_locations
          .entry(r#type)
          .or_default()
          .push(fun.location);
      } else {
        tracing::error!("Found an invalid `borrow`: `{:?}`{location_hint}. Borrows must be of form `borrow = \"namespace::Type\"`", fun.borrow, location_hint = utils::display_code_location(Some(&vec![fun.location])));
        exit(1);
      }
    }
  }
}

/// Construct a `Membrane` from a completed `RegistryData`.
pub(crate) fn into_membrane(data: RegistryData) -> Membrane {
  Membrane {
    errors: data.errors,
    package_name: match std::env::var_os("MEMBRANE_PACKAGE_NAME") {
      Some(name) => name.to_string_lossy().into_owned(),
      None => String::new(),
    },
    destination: match std::env::var_os("MEMBRANE_DESTINATION") {
      Some(dest) => PathBuf::from(dest),
      None => PathBuf::from("membrane_output"),
    },
    library: match std::env::var_os("MEMBRANE_LIBRARY") {
      Some(library) => library.to_string_lossy().into_owned(),
      None => "libmembrane".to_string(),
    },
    llvm_paths: match std::env::var_os("MEMBRANE_LLVM_PATHS") {
      Some(config) => config
        .to_string_lossy()
        .split(&[',', ' '][..])
        .map(|x| x.to_string())
        .collect(),
      None => vec![],
    },
    namespaced_registry: data.namespaced_registry,
    namespaced_fn_registry: data.namespaced_fn_registry,
    namespaced_enum_registry: data.namespaced_enum_registry,
    namespaces: data.namespaces,
    generated: false,
    c_style_enums: true,
    sealed_enums: true,
    timeout: None,
    borrows: data.borrows,
    _inputs: data.input_libs,
    dart_config: DartConfig::default(),
  }
}
