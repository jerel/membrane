//!
//! ## Overview
//! Membrane works by inspecting public functions and their types at
//! compile time, then generating corresponding Dart code and C bindings capable of
//! calling those functions and representing their data. To mark a Rust function as accessible to Dart we
//! just apply the `#[async_dart]` macro, Membrane handles everything else.
//!
//! Functions published to Dart use Rust types for their parameters including scalar types, structs, or enums -
//! no need to work with C types in your Rust code.
//! Both async functions and streams are supported, return values are handled via a zero-copy buffer
//! using the bincode encoding (bincode is very efficient because it encodes data only as opposed to structure + data).
//! Membrane is being used by a large project with stringent performance
//! requirements and with this zero-copy encoding approach we were able to achieve zero
//! frame drops in Flutter while transferring significant amounts of data from Rust.
//!
//! ### Usage
//! Since Membrane needs to compile the project and then run code to inspect types we need
//! a "post build" task to run Membrane, separate from your `cargo build` step. This can
//! be accomplished by adding a `generator.rs` (or similar) bin file to your project.
//!
//! Example:
//! ```
//! // lib.rs
//! #[async_dart(namespace = "accounts")]
//! pub async fn update_user(id: i64, user: User) -> Result<User, String> {
//!   todo!()
//! }
//!
//! #[async_dart(namespace = "accounts")]
//! pub fn users() -> impl Stream<Item = Result<User, MyError>> {
//!   futures::stream::iter(vec![Ok(User::default())])
//! }
//! ```
//!
//! ```
//! // bin.rs
//! fn main() {
//!   let mut project = membrane::Membrane::new();
//!   project
//!     .package_destination_dir("../dart_example")
//!     .package_name("example")
//!     .using_lib("libexample")
//!     .create_pub_package()
//!     .write_api()
//!     .write_c_headers()
//!     .write_bindings();
//! }
//! # static SILENCE_CLIPPY_MAIN = 1;
//! ```
//! For a runnable example see the [`example`](https://github.com/jerel/membrane/tree/main/example) directory
//! and run `cargo run` to inspect the Dart output in the `dart_example` directory.
//!
//! By default Membrane stores metadata during the compile step whenever the project is
//! compiled in debug mode. This has two implications:
//! 1. `cargo run --bin generator --release` won't work.
//! 1. A library compiled in `release` mode will have no Membrane metadata in the resulting binary.
//!
//! If you need to force a different behavior the feature flags `skip-generate` and `generate` are
//! available to override the default behavior.
//!

#[doc(hidden)]
pub use allo_isolate;
#[doc(hidden)]
pub use bincode;
#[doc(hidden)]
pub use ffi_helpers;
#[doc(hidden)]
pub use futures;
#[doc(hidden)]
pub use inventory;
#[doc(hidden)]
pub use membrane_macro::{async_dart, dart_enum, sync_dart};
#[doc(hidden)]
pub use serde_reflection;

#[doc(hidden)]
pub mod emitter;
#[doc(hidden)]
pub mod metadata;
pub mod runtime;
#[doc(hidden)]
pub mod utils;

mod generators;

use generators::{
  functions::{Builder, Writable},
  loaders,
};
use membrane_types::heck::{CamelCase, SnakeCase};
use serde_reflection::{
  ContainerFormat, Error, Registry, Samples, Tracer, TracerConfig, VariantFormat,
};
use std::{
  collections::{BTreeMap, BTreeSet, HashMap},
  fs::{read_to_string, remove_file},
  io::Write,
  path::{Path, PathBuf},
  process::exit,
};
use tracing::{info, warn};

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
}

#[doc(hidden)]
#[derive(Clone)]
pub struct DeferredTrace {
  pub function: Function,
  pub namespace: &'static str,
  pub trace: fn(tracer: &mut serde_reflection::Tracer, samples: &mut serde_reflection::Samples),
}

impl std::fmt::Debug for DeferredTrace {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("DeferredTrace")
      .field("function", &self.function)
      .field("namespace", &self.namespace)
      .finish()
  }
}

#[doc(hidden)]
#[derive(Clone)]
pub struct DeferredEnumTrace {
  pub name: &'static str,
  pub namespace: &'static str,
  pub trace: fn(tracer: &mut serde_reflection::Tracer),
}

impl std::fmt::Debug for DeferredEnumTrace {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("DeferredEnumTrace")
      .field("name", &self.name)
      .field("namespace", &self.namespace)
      .finish()
  }
}

inventory::collect!(DeferredTrace);
inventory::collect!(DeferredEnumTrace);

pub struct Membrane {
  package_name: String,
  destination: PathBuf,
  library: String,
  llvm_paths: Vec<String>,
  namespaces: Vec<&'static str>,
  namespaced_registry: HashMap<&'static str, serde_reflection::Result<Registry>>,
  namespaced_fn_registry: HashMap<&'static str, Vec<Function>>,
  generated: bool,
  c_style_enums: bool,
  timeout: Option<i32>,
  borrows: HashMap<&'static str, BTreeMap<&'static str, BTreeSet<&'static str>>>,
  _inputs: Vec<libloading::Library>,
}

impl<'a> Membrane {
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let mut input_libs = vec![];

    std::env::set_var(
      "RUST_LOG",
      std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string()),
    );

    let _ = pretty_env_logger::try_init();

    // collect all libexample.so paths from stdin
    let lib_paths: Vec<String> = std::env::args().skip(1).collect();

    let (enums, functions) = if lib_paths.is_empty() {
      info!("No `lib.so` paths were passed via stdin, falling back to looking for types in the local `lib` source");
      (metadata::enums(), metadata::functions())
    } else {
      lib_paths.iter().fold(
        (vec![], vec![]),
        |(mut acc_enums, mut acc_functions), path| {
          let (enums, functions) = utils::extract_types_from_cdylib(path, &mut input_libs);
          acc_enums.extend(enums);
          acc_functions.extend(functions);
          (acc_enums, acc_functions)
        },
      )
    };

    if enums.is_empty() && functions.is_empty() {
      info!(
        "No type information could be found. Do you have #[async_dart] or #[sync_dart] in your code?"
      );
    }

    let mut namespaces = vec![
      enums.iter().map(|x| x.namespace).collect::<Vec<&str>>(),
      functions.iter().map(|x| x.namespace).collect::<Vec<&str>>(),
    ]
    .concat();

    namespaces.sort();
    namespaces.dedup();

    let mut namespaced_registry = HashMap::new();
    let mut namespaced_samples = HashMap::new();
    let mut namespaced_fn_registry = HashMap::new();
    let mut borrows: HashMap<&str, BTreeMap<&str, BTreeSet<&str>>> = HashMap::new();

    // collect all the metadata about functions (without tracing them yet)
    functions.iter().for_each(|item| {
      namespaced_fn_registry
        .entry(item.namespace.clone())
        .or_insert_with(Vec::new)
        .push(item.function.clone());
    });

    // work out which namespaces borrow which types from other namespaces
    namespaces.iter().for_each(|namespace| {
      Self::create_borrows(&namespaced_fn_registry, namespace, &mut borrows);
    });

    // trace all the enums at least once
    enums.iter().for_each(|item| {
      // trace the enum into the borrowing namespace's registry
      borrows
        .clone()
        .into_iter()
        .for_each(|(for_namespace, from_namespaces)| {
          if let Some(types) = from_namespaces.get(item.namespace) {
            if types.contains(item.name) {
              let tracer = namespaced_registry
                .entry(for_namespace)
                .or_insert_with(|| Tracer::new(TracerConfig::default()));

              (item.trace)(tracer);
            }
          }
        });

      // trace the enum into the owning namespace's registry
      let tracer = namespaced_registry
        .entry(item.namespace)
        .or_insert_with(|| Tracer::new(TracerConfig::default()));

      (item.trace)(tracer);
    });

    // now that we have the enums in the registry we'll trace each of the functions
    functions.iter().for_each(|item| {
      let tracer = namespaced_registry
        .entry(item.namespace)
        .or_insert_with(|| Tracer::new(TracerConfig::default()));

      let samples = namespaced_samples
        .entry(item.namespace.clone())
        .or_insert_with(Samples::new);

      (item.trace)(tracer, samples);
    });

    Self {
      package_name: match std::env::var_os("MEMBRANE_PACKAGE_NAME") {
        Some(name) => name.into_string().unwrap(),
        None => "".to_string(),
      },
      destination: match std::env::var_os("MEMBRANE_DESTINATION") {
        Some(dest) => PathBuf::from(dest),
        None => PathBuf::from("membrane_output"),
      },
      library: match std::env::var_os("MEMBRANE_LIBRARY") {
        Some(library) => library.into_string().unwrap(),
        None => "libmembrane".to_string(),
      },
      llvm_paths: match std::env::var_os("MEMBRANE_LLVM_PATHS") {
        Some(config) => config
          .into_string()
          .unwrap()
          .split(&[',', ' '][..])
          .map(|x| x.to_string())
          .collect(),
        None => vec![],
      },
      namespaced_registry: namespaced_registry
        .into_iter()
        .map(|(key, val)| (key, val.registry()))
        .collect(),
      namespaced_fn_registry,
      namespaces,
      generated: false,
      c_style_enums: true,
      timeout: None,
      borrows,
      _inputs: input_libs,
    }
  }

  ///
  /// The directory for the pub package output. The basename will be the name of the pub package unless `package_name` is used.
  ///
  /// Can be overridden with the environment variable `MEMBRANE_DESTINATION`.
  pub fn package_destination_dir<P: ?Sized + AsRef<Path>>(&mut self, path: &'a P) -> &mut Self {
    // allowing an empty path could result in data loss in a directory named `lib`
    assert!(
      !path.as_ref().to_str().unwrap().is_empty(),
      "package_destination_dir() cannot be called with an empty path"
    );
    if self.destination == PathBuf::from("membrane_output") {
      self.destination = path.as_ref().to_path_buf();
    }
    self
  }

  ///
  /// The name of the generated package.
  ///
  /// Can be overridden with the environment variable `MEMBRANE_PACKAGE_NAME`.
  pub fn package_name(&mut self, name: &str) -> &mut Self {
    if self.package_name.is_empty() {
      self.package_name = name.to_string();
    }
    self
  }

  ///
  /// Paths to search (at build time) for the libclang library.
  ///
  /// Can be overridden with the environment variable `MEMBRANE_LLVM_PATHS`. Takes a comma or space separated list.
  pub fn llvm_paths(&mut self, paths: Vec<&str>) -> &mut Self {
    assert!(
      !paths.is_empty(),
      "llvm_paths() cannot be called with no paths"
    );
    if self.llvm_paths.is_empty() {
      self.llvm_paths = paths.iter().map(|x| x.to_string()).collect();
    }
    self
  }

  ///
  /// The name (without the extension) of the `dylib` or `so` that the Rust project produces. Membrane
  /// generated code will load this library at runtime.
  ///
  /// Can be overridden with the environment variable `MEMBRANE_LIBRARY`.
  pub fn using_lib(&mut self, name: &str) -> &mut Self {
    if self.library == "libmembrane" {
      self.library = name.to_string();
    }
    self
  }

  ///
  /// Write the pub package to the destination set with `package_destination_dir`.
  /// Existing Dart files in this directory may be deleted during this operation.
  #[allow(unreachable_code)]
  pub fn create_pub_package(&mut self) -> &mut Self {
    use serde_generate::SourceInstaller;

    #[cfg(all(
      any(not(debug_assertions), feature = "skip-generate"),
      not(feature = "generate")
    ))]
    return self;

    // remove all previously generated type and header files
    let _ = std::fs::remove_dir_all(self.destination.join("lib"));
    let _ = std::fs::remove_file(self.destination.join("pubspec.yaml"));
    std::fs::create_dir_all(self.destination.join("lib").join("src")).unwrap();

    let installer = serde_generate::dart::Installer::new(self.destination.to_path_buf());
    installer.install_serde_runtime().unwrap();
    installer.install_bincode_runtime().unwrap();

    for namespace in self.namespaces.iter() {
      info!("Generating lib/src/ code for namespace {}", namespace);
      let config = serde_generate::CodeGeneratorConfig::new(namespace.to_string())
        .with_encodings(vec![serde_generate::Encoding::Bincode])
        .with_c_style_enums(self.c_style_enums);

      let registry = match self.namespaced_registry.get(namespace).unwrap() {
        Ok(reg) => reg,
        Err(Error::MissingVariants(names)) => {
          tracing::error!(
            "An enum was used that has not had the membrane::dart_enum macro applied for the consuming namespace. Please add #[dart_enum(namespace = \"{}\")] to the {} enum.",
            namespace,
            names.first().unwrap()
          );
          exit(1);
        }
        Err(err) => panic!("{}", err),
      };
      let generator = serde_generate::dart::CodeGenerator::new(&config);
      generator
        .output(self.destination.to_path_buf(), registry)
        .unwrap();
    }

    self.generated = true;
    self.write_pubspec();

    let pub_get = std::process::Command::new("dart")
      .current_dir(&self.destination)
      .arg("--disable-analytics")
      .arg("pub")
      .arg("get")
      .arg("--precompile")
      .output()
      .unwrap();

    if pub_get.status.code() != Some(0) {
      std::io::stderr().write_all(&pub_get.stderr).unwrap();
      std::io::stdout().write_all(&pub_get.stdout).unwrap();
      tracing::error!("'dart pub get' returned an error");
    }

    self
  }

  ///
  /// When set to `true` (the default) we generate basic Dart enums. When set to `false`
  /// Dart classes are generated (one for the base case and one for each variant).
  pub fn with_c_style_enums(&mut self, val: bool) -> &mut Self {
    self.c_style_enums = val;
    self
  }

  ///
  /// Configures the global timeout for non-stream receive ports.
  /// Streams do not use the global timeout as it is unusual to want a stream to timeout
  /// between events. All timeouts can be set at the function level by passing
  /// `timeout = 5000` as an option to `async_dart`.
  ///
  /// Default: 1000ms
  pub fn timeout(&mut self, val: i32) -> &mut Self {
    self.timeout = Some(val);
    self
  }

  ///
  /// Write a header file for each namespace that provides the C types
  /// needed by ffigen to generate the FFI bindings.
  pub fn write_c_headers(&mut self) -> &mut Self {
    let head = r#"/*
 * AUTO GENERATED FILE, DO NOT EDIT
 *
 * Generated by `membrane`
 */
#include <stdint.h>

#ifndef __MEMBRANE_TYPES_INCLUDED__
#define __MEMBRANE_TYPES_INCLUDED__

typedef enum MembraneMsgKind {
  Ok,
  Error,
} MembraneMsgKind;

typedef enum MembraneResponseKind {
  Data,
  Panic,
} MembraneResponseKind;

typedef struct MembraneResponse
{
  uint8_t kind;
  const void *data;
} MembraneResponse;

uint8_t membrane_cancel_membrane_task(const void *task_handle);
uint8_t membrane_free_membrane_vec(int64_t len, const void *ptr);

#endif
"#;

    let path = self.destination.join("lib/src/membrane_types.h");
    std::fs::write(&path, head).unwrap_or_else(|_| {
      tracing::error!("unable to write {}", path.to_str().unwrap());
      exit(1);
    });

    let namespaces = self.namespaces.clone();
    namespaces.iter().for_each(|x| {
      self.write_header(x);
    });

    self
  }

  ///
  /// Write all Dart classes needed by the Dart application.
  pub fn write_api(&mut self) -> &mut Self {
    let namespaces = self.namespaces.clone();
    namespaces.iter().for_each(|x| {
      self.create_ffi_impl(x);
      self.create_web_impl(x);
      self.create_class(x.to_string());
    });

    self.create_imports();

    if self.generated {
      self.create_loader();
      self.format_package();
    }

    self
  }

  ///
  /// Invokes `dart run ffigen` with the appropriate config to generate FFI bindings.
  pub fn write_bindings(&mut self) -> &mut Self {
    if !self.generated {
      return self;
    }

    self.write_ffigen_config();

    let ffigen = std::process::Command::new("dart")
      .current_dir(&self.destination)
      .arg("--disable-analytics")
      .arg("run")
      .arg("ffigen")
      .arg("--config")
      .arg("ffigen.yaml")
      .output()
      .unwrap();

    if ffigen.status.code() != Some(0) {
      std::io::stderr().write_all(&ffigen.stderr).unwrap();
      std::io::stdout().write_all(&ffigen.stdout).unwrap();
      tracing::error!("dart ffigen returned an error");
      exit(1);
    }

    self
  }

  ///
  /// Private implementations
  ///

  fn write_pubspec(&mut self) -> &mut Self {
    // serde-generate uses the last namespace as the pubspec name and dart doesn't
    // like that so we set a proper package name from the basename or from an explicitly given name
    let package_name = if self.package_name.is_empty() {
      self
        .destination
        .to_path_buf()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string()
    } else {
      self.package_name.as_str().to_string()
    };
    let path = self.destination.join("pubspec.yaml");

    if let Ok(old) = std::fs::read_to_string(&path) {
      let pubspec = old
        .lines()
        .filter(|l| !l.is_empty())
        .map(|ln| {
          if ln.contains("name:") {
            format!("name: {}", package_name)
          } else if ln.contains("sdk:") {
            // ffigen >= 5 requires dart >= 2.17, so replace dart version from serde-reflection
            "  sdk: '>=2.17.0 <3.0.0'".to_owned()
          } else {
            ln.to_owned()
          }
        })
        .chain(vec![
          "  ffi: ^2.0.0".to_owned(),
          "  logging: ^1.0.2".to_owned(),
          "dev_dependencies:".to_owned(),
          "  ffigen: ^6.1.2\n".to_owned(),
        ])
        .collect::<Vec<String>>()
        .join("\n");

      std::fs::write(path, pubspec).expect("pubspec could not be written");
      let _ = std::fs::remove_file(self.destination.join("pubspec.lock"));
    }

    self
  }

  fn write_ffigen_config(&mut self) -> &mut Self {
    let config = format!(
      r#"name: 'NativeLibrary'
description: 'Auto generated bindings for Dart types'
output: './lib/src/ffi_bindings.dart'
sort: true
enums:
  include:
    - MembraneMsgKind
    - MembraneResponseKind
  member-rename:
    'Membrane(.*)':
      'Data': 'data'
      'Error': 'error'
      'Ok': 'ok'
      'Panic': 'panic'
macros:
  include:
    - __none__
structs:
  include:
    - MembraneResponse
unions:
  include:
    - __none__
unnamed-enums:
  include:
    - __none__
headers:
  entry-points:
    - 'lib/src/membrane_types.h'
    - 'lib/src/*/*.h'
{}
"#,
      if !self.llvm_paths.is_empty() {
        "llvm-path:".to_string()
          + &self
            .llvm_paths
            .iter()
            .map(|p| "\n  - '".to_string() + p + "'")
            .collect::<Vec<String>>()
            .join("")
      } else {
        "".to_string()
      }
    );

    let path = self.destination.join("ffigen.yaml");
    std::fs::write(&path, config).unwrap_or_else(|_| {
      tracing::error!("unable to write ffigen config {}", path.to_str().unwrap());
      exit(1);
    });

    self
  }

  fn write_header(&mut self, namespace: &str) -> &mut Self {
    use std::io::prelude::*;
    let path = self
      .namespace_path(namespace)
      .join(namespace.to_string() + ".h");
    let default = &vec![];
    let fns = self
      .namespaced_fn_registry
      .get(namespace)
      .unwrap_or(default);

    let head = r#"/*
 * AUTO GENERATED FILE, DO NOT EDIT
 *
 * Generated by `membrane`
 */
#include <stdint.h>
#include "../membrane_types.h"
"#;

    let mut buffer =
      std::fs::File::create(path.clone()).expect("header could not be written at namespace path");
    buffer.write_all(head.as_bytes()).unwrap_or_else(|_| {
      tracing::error!("unable to write C header file {}", path.to_str().unwrap());
      exit(1);
    });

    fns.iter().for_each(|x| {
      generators::functions::C::new(x).build(self).write(&buffer);
    });

    self
  }

  fn format_package(&mut self) -> &mut Self {
    // quietly attempt a code format if dart is installed
    let _ = std::process::Command::new("dart")
      .current_dir(&self.destination)
      .arg("--disable-analytics")
      .arg("format")
      .arg(".")
      .output();

    self
  }

  fn create_loader(&mut self) -> &mut Self {
    let ffi_loader = loaders::create_ffi_loader(&self.library);
    let path = self.destination.join("lib/src/membrane_loader_ffi.dart");
    std::fs::write(path, ffi_loader).unwrap();

    let web_loader = loaders::create_web_loader(&self.library);
    let path = self.destination.join("lib/src/membrane_loader_web.dart");
    std::fs::write(path, web_loader).unwrap();

    let barrel_loader = "// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
export './membrane_loader_ffi.dart' if (dart.library.html) './membrane_loader_web.dart';";

    let path = self.destination.join("lib/src/membrane_loader.dart");
    std::fs::write(path, barrel_loader).unwrap();

    self
  }

  fn create_class(&mut self, namespace: String) -> &mut Self {
    use std::io::prelude::*;
    let path = self
      .destination
      .join("lib")
      .join(namespace.to_string() + ".dart");

    let head = format!(
      r#"// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
export './src/{ns}_ffi.dart' if (dart.library.html) './src/{ns}_web.dart';
"#,
      ns = &namespace,
    );

    let mut buffer = std::fs::File::create(path).expect("class could not be written at path");
    buffer.write_all(head.as_bytes()).unwrap();

    self
  }

  fn create_ffi_impl(&mut self, namespace: &str) -> &mut Self {
    use std::io::prelude::*;
    let path = self
      .destination
      .join("lib/src")
      .join(namespace.to_string() + "_ffi.dart");

    if self.namespaced_fn_registry.get(namespace).is_none() {
      let head = format!(
        "export './{ns}/{ns}.dart' hide TraitHelpers;",
        ns = &namespace
      );
      let mut buffer = std::fs::File::create(path).expect("class could not be written at path");
      buffer.write_all(head.as_bytes()).unwrap();

      return self;
    }

    let default = &vec![];
    let fns = self
      .namespaced_fn_registry
      .get(&namespace)
      .unwrap_or(default);

    let head = format!(
      r#"// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
import 'dart:ffi';
import 'dart:isolate' show ReceivePort;
import 'dart:typed_data';
import 'package:ffi/ffi.dart';
import 'package:logging/logging.dart';
import 'package:meta/meta.dart';

import './membrane_loader.dart' as loader;
import './bincode/bincode.dart';
import './ffi_bindings.dart' show MembraneMsgKind, MembraneResponse, MembraneResponseKind;
import './{ns}/{ns}.dart';

export './{ns}/{ns}.dart' hide TraitHelpers;

final _bindings = loader.bindings;
final _loggingDisabled = bool.fromEnvironment('MEMBRANE_DISABLE_LOGS');

@immutable
class {class_name}ApiError implements Exception {{
  final e;
  const {class_name}ApiError(this.e);
}}

@immutable
class {class_name}Api {{
  static final _log = Logger('membrane.{ns}');
  const {class_name}Api();
"#,
      ns = &namespace,
      class_name = &namespace.to_camel_case()
    );

    let mut buffer = std::fs::File::create(path).expect("class could not be written at path");
    buffer.write_all(head.as_bytes()).unwrap();

    fns.iter().for_each(|x| {
      generators::functions::Ffi::new(x)
        .build(self)
        .write(&buffer);
    });

    buffer.write_all(b"}\n").unwrap();

    self
  }

  fn create_web_impl(&mut self, namespace: &str) -> &mut Self {
    use std::io::prelude::*;
    let path = self
      .destination
      .join("lib/src")
      .join(namespace.to_string() + "_web.dart");

    // perhaps this namespace has only enums in it and no functions
    if self.namespaced_fn_registry.get(namespace).is_none() {
      let head = format!(
        "export './{ns}/{ns}.dart' hide TraitHelpers;",
        ns = &namespace
      );
      let mut buffer = std::fs::File::create(path).expect("class could not be written at path");
      buffer.write_all(head.as_bytes()).unwrap();

      return self;
    }

    let default = &vec![];
    let fns = self
      .namespaced_fn_registry
      .get(&namespace)
      .unwrap_or(default);

    let head = format!(
      r#"// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
import 'package:meta/meta.dart';
import './{ns}/{ns}.dart';
export './{ns}/{ns}.dart' hide TraitHelpers;

@immutable
class {class_name}ApiError implements Exception {{
  final e;
  const {class_name}ApiError(this.e);
}}

@immutable
class {class_name}Api {{
  const {class_name}Api();
"#,
      ns = &namespace,
      class_name = &namespace.to_camel_case()
    );

    let mut buffer = std::fs::File::create(path).expect("class could not be written at path");
    buffer.write_all(head.as_bytes()).unwrap();

    fns.iter().for_each(|x| {
      generators::functions::Web::new(x)
        .build(self)
        .write(&buffer);
    });

    buffer.write_all(b"}\n").unwrap();

    self
  }

  fn namespace_path(&mut self, namespace: &str) -> PathBuf {
    self.destination.join("lib").join("src").join(&namespace)
  }

  fn create_borrows(
    namespaced_fn_registry: &HashMap<&str, Vec<Function>>,
    namespace: &'static str,
    borrows: &mut HashMap<&'static str, BTreeMap<&str, BTreeSet<&str>>>,
  ) {
    let default = &vec![];
    let fns = namespaced_fn_registry.get(namespace).unwrap_or(default);

    fns.iter().for_each(move |fun| {
      fun
        .borrow
        .iter()
        .map(|borrow| borrow.split("::").map(|x| x.trim()).collect::<Vec<&str>>())
        .for_each(|borrow_list| {
          if let [from_namespace, r#type] = borrow_list[..] {
            let imports = borrows.entry(namespace).or_default();
            let types = imports.entry(from_namespace).or_default();
            types.insert(r#type);
          } else {
            tracing::error!("Found an invalid `borrow`: `{:?}`. Borrows must be of form `borrow = \"namespace::Type\"`", fun.borrow);
            exit(1);
          }
        });
    });
  }

  fn create_imports(&mut self) -> &mut Self {
    self.borrows.iter().for_each(|(namespace, imports)| {
      imports
        .iter()
        // sort the imports in reverse order so that we can append them to existing
        // lines and up with a descending order
        .rev()
        .for_each(|(from_namespace, borrowed_types)| {
          let mut borrowed_types: Vec<String> = borrowed_types.iter().flat_map(|r#type| {
            let auto_import = self.with_child_borrows(from_namespace, r#type);
            auto_import.iter().for_each(|x| {
              if borrowed_types.contains(x.as_str()) && x != r#type {
                warn!("{ns}::{import} was explicitly borrowed but it is already implicitly borrowed because it is a subtype of `{ns}::{type}`. Remove the `{ns}::{import}` borrow.",
                ns = from_namespace, import = x, r#type = r#type);
              }
            });

            auto_import
          }).collect();

          borrowed_types.sort();
          borrowed_types.dedup();

          let file_name = format!("{ns}.dart", ns = namespace);
          let namespace_path = self.destination.join("lib/src").join(namespace);
          let barrel_file_path = namespace_path.join(file_name);

          let barrel_file = read_to_string(&barrel_file_path)
            .unwrap()
            .lines()
            .filter_map(|line| {
              if borrowed_types.contains(
                &line
                  .replace("part '", "")
                  .replace(".dart';", "")
                  .to_camel_case(),
              ) {
                None
              } else if line.starts_with("import '../bincode") {
                Some(vec![
                  line.to_string(),
                  format!(
                    "import '../{ns}/{ns}.dart' show {types};",
                    ns = from_namespace,
                    types = borrowed_types.join(",")
                  ),
                ])
              } else if line.starts_with("export '../serde") {
                Some(vec![
                  line.to_string(),
                  format!(
                    "export '../{ns}/{ns}.dart' show {types};",
                    ns = from_namespace,
                    types = borrowed_types.join(",")
                  ),
                ])
              } else {
                Some(vec![line.to_string()])
              }
            })
            .flatten()
            .collect::<Vec<String>>();

          borrowed_types.iter().for_each(|borrowed_type| {
            let filename = format!("{}.dart", borrowed_type.to_snake_case());
            let _ = remove_file(namespace_path.join(filename));
          });

          std::fs::write(barrel_file_path, barrel_file.join("\n")).unwrap();
        });
    });

    self
  }
}

#[doc(hidden)]
#[repr(u8)]
#[derive(serde::Serialize)]
pub enum MembraneResponseKind {
  Data,
  Panic,
}

#[doc(hidden)]
#[repr(C)]
pub struct MembraneResponse {
  pub kind: MembraneResponseKind,
  pub data: *const std::ffi::c_void,
}

#[doc(hidden)]
#[repr(u8)]
#[derive(serde::Serialize)]
pub enum MembraneMsgKind {
  Ok,
  Error,
}

#[doc(hidden)]
pub struct TaskHandle(pub Box<dyn Fn()>);

#[doc(hidden)]
#[no_mangle]
pub unsafe extern "C" fn membrane_cancel_membrane_task(task_handle: *mut TaskHandle) -> i32 {
  // turn the pointer back into a box and Rust will drop it when it goes out of scope
  let handle = Box::from_raw(task_handle);
  (handle.0)();

  1
}

#[doc(hidden)]
#[no_mangle]
pub unsafe extern "C" fn membrane_free_membrane_vec(len: i64, ptr: *const u8) -> i32 {
  // turn the pointer back into a vec and Rust will drop it
  let _ = ::std::slice::from_raw_parts::<u8>(ptr, len as usize);

  1
}

#[doc(hidden)]
#[macro_export]
macro_rules! error {
  ($result:expr) => {
    error!($result, ::std::ptr::null());
  };
  ($result:expr, $error:expr) => {
    match $result {
      Ok(value) => value,
      Err(e) => {
        ::membrane::ffi_helpers::update_last_error(e);
        // silence unreachable code warnings to enable panicking on invalid data
        #[allow(unreachable_code)]
        return $error;
      }
    }
  };
}

#[doc(hidden)]
#[macro_export]
macro_rules! cstr {
  ($ptr:expr) => {
    cstr!($ptr, ::std::ptr::null())
  };
  ($ptr:expr, $error:expr) => {{
    ::membrane::ffi_helpers::null_pointer_check!($ptr);
    error!(unsafe { CStr::from_ptr($ptr).to_str() }, $error)
  }};
}

#[cfg(test)]
mod tests {
  use std::env::{remove_var, set_var};
  use std::path::PathBuf;

  use crate::Membrane;

  #[test]
  fn test_envars_are_used() {
    let project = Membrane::new();
    assert_eq!(project.package_name, "");
    assert_eq!(project.destination, PathBuf::from("membrane_output"));
    assert_eq!(project.library, "libmembrane");
    assert!(project.llvm_paths.is_empty());

    set_var("MEMBRANE_PACKAGE_NAME", "a_package");
    set_var("MEMBRANE_DESTINATION", "./this_dir");
    set_var("MEMBRANE_LIBRARY", "libcustom");
    set_var("MEMBRANE_LLVM_PATHS", "/usr/lib/opt/foo,/usr/lib/opt/bar");

    let project2 = Membrane::new();
    assert_eq!(project2.package_name, "a_package");
    assert_eq!(project2.destination, PathBuf::from("./this_dir"));
    assert_eq!(project2.library, "libcustom");
    assert_eq!(
      project2.llvm_paths,
      vec!["/usr/lib/opt/foo", "/usr/lib/opt/bar"]
    );

    remove_var("MEMBRANE_PACKAGE_NAME");
    remove_var("MEMBRANE_DESTINATION");
    remove_var("MEMBRANE_LIBRARY");
    remove_var("MEMBRANE_LLVM_PATHS");
  }
}
