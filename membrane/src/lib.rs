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
pub use git_version::git_version;
#[doc(hidden)]
pub use inventory;
#[doc(hidden)]
pub use membrane_macro::{async_dart, dart_enum, export_metadata, sync_dart};
#[doc(hidden)]
pub use serde_reflection;

pub mod config;
#[doc(hidden)]
pub mod emitter;
#[doc(hidden)]
pub mod ffi_types;
#[doc(hidden)]
pub mod metadata;
#[doc(hidden)]
pub mod registry;
pub mod runtime;
#[doc(hidden)]
pub mod utils;

mod generators;

use generators::{
  exceptions,
  functions::{Builder, Writable},
  imports, loaders,
};
use membrane_types::heck::{ToSnakeCase, ToUpperCamelCase};
use serde_reflection::{ContainerFormat, Error, Registry, VariantFormat};
use std::{
  collections::HashMap,
  fs::remove_file,
  io::Write,
  path::{Path, PathBuf},
  process::exit,
};
use tracing::{debug, warn};

pub use config::{DartConfig, DartLoggerConfig};
#[doc(hidden)]
pub use registry::{DeferredEnumTrace, DeferredTrace, Enum, Function};

use registry::{Borrows, SourceCodeLocation};

macro_rules! return_if_error {
  ( $e:expr ) => {
    if !$e.errors.is_empty() {
      return $e;
    }
  };
}

#[derive(Debug)]
pub struct Membrane {
  errors: Vec<String>,
  package_name: String,
  destination: PathBuf,
  library: String,
  llvm_paths: Vec<String>,
  namespaces: Vec<&'static str>,
  namespaced_registry: HashMap<&'static str, serde_reflection::Result<Registry>>,
  namespaced_fn_registry: HashMap<&'static str, Vec<Function>>,
  namespaced_enum_registry: HashMap<&'static str, Vec<Enum>>,
  generated: bool,
  c_style_enums: bool,
  sealed_enums: bool,
  timeout: Option<i32>,
  borrows: Borrows,
  _inputs: Vec<libloading::Library>,
  dart_config: DartConfig,
}

impl<'a> Membrane {
  ///
  /// This method should be used when your project imports the crate's `lib` source code into the
  /// `bin` where Membrane is initialized.
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    Self::initialize_from_metadata(None::<&std::path::Path>)
  }

  ///
  /// This method loads the .so produced by `cargo build` and extracts type information from it.
  /// This is a more performant approach than `Membrane::new()` as it does not do any recompilation of
  /// application code or dependencies.
  ///
  /// At compile time (`cargo build --lib`) the Cargo and Git versions from your
  /// source repository are stored in the lib. You can override the default version information by exporting
  /// the `MEMBRANE_CDYLIB_VERSION="1.0-my-version"` environment variable before doing the
  /// cargo build of the cdylib. This version information is logged during code generation and also
  /// when Dart loads the cdylib at runtime.
  pub fn new_from_cdylib<P>(cdylib_path: &'a P) -> Self
  where
    P: AsRef<Path> + std::fmt::Debug,
  {
    Self::initialize_from_metadata(Some(cdylib_path))
  }

  fn initialize_from_metadata<P>(cdylib_path: Option<&'a P>) -> Self
  where
    P: ?Sized + AsRef<Path> + std::fmt::Debug,
  {
    std::env::set_var(
      "RUST_LOG",
      std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
    );

    let _ = pretty_env_logger::try_init();

    registry::into_membrane(registry::build(cdylib_path))
  }

  ///
  /// The directory for the pub package output. The basename will be the name of the pub package unless `package_name` is used.
  ///
  /// Can be overridden with the environment variable `MEMBRANE_DESTINATION`.
  pub fn package_destination_dir<P: ?Sized + AsRef<Path>>(&mut self, path: &'a P) -> &mut Self {
    return_if_error!(self);
    // allowing an empty path could result in data loss in a directory named `lib`
    assert!(
      !path.as_ref().to_str().unwrap().is_empty(),
      "package_destination_dir() cannot be called with an empty path"
    );
    // compatibility with rust 1.84
    #[allow(clippy::cmp_owned)]
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
    return_if_error!(self);
    if self.package_name.is_empty() {
      self.package_name = name.to_string();
    }
    self
  }

  ///
  /// Paths to search (at build time) for the libclang library.
  ///
  /// Can be overridden with the environment variable `MEMBRANE_LLVM_PATHS`. Takes a comma or space separated list.
  pub fn llvm_paths(&mut self, paths: &[&str]) -> &mut Self {
    return_if_error!(self);
    assert!(
      !paths.is_empty(),
      "llvm_paths() cannot be called with no paths"
    );
    if self.llvm_paths.is_empty() {
      self.llvm_paths = paths.iter().map(ToString::to_string).collect();
    }
    self
  }

  ///
  /// The name (without the extension) of the `dylib` or `so` that the Rust project produces. Membrane
  /// generated code will load this library at runtime.
  ///
  /// Can be overridden with the environment variable `MEMBRANE_LIBRARY`.
  pub fn using_lib(&mut self, name: &str) -> &mut Self {
    return_if_error!(self);
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
    return_if_error!(self);

    #[cfg(all(
      any(not(debug_assertions), feature = "skip-generate"),
      not(feature = "generate")
    ))]
    return self;

    // remove all previously generated type and header files
    let _ = std::fs::remove_dir_all(self.destination.join("lib"));
    let _ = std::fs::remove_file(self.destination.join("pubspec.yaml"));
    if let Err(e) = std::fs::create_dir_all(self.destination.join("lib").join("src")) {
      self
        .errors
        .push(format!("unable to create output directory: {e}"));
      return self;
    }

    let installer = serde_generate::dart::Installer::new(self.destination.clone());
    if let Err(e) = installer.install_serde_runtime() {
      self
        .errors
        .push(format!("unable to install serde runtime: {e}"));
      return self;
    }
    if let Err(e) = installer.install_bincode_runtime() {
      self
        .errors
        .push(format!("unable to install bincode runtime: {e}"));
      return self;
    }

    for namespace in &self.namespaces {
      debug!("Generating lib/src/ code for namespace {}", namespace);
      let config = serde_generate::CodeGeneratorConfig::new(namespace.to_string())
        .with_encodings(vec![serde_generate::Encoding::Bincode])
        .with_c_style_enums(self.c_style_enums)
        .with_sealed_enums(self.sealed_enums)
        .with_enum_type_overrides(
          self
            .namespaced_enum_registry
            .get(namespace)
            .into_iter()
            .flat_map(|enums| {
              enums
                .iter()
                .filter_map(|x| x.output.map(|output| (x.name, output)))
            })
            .collect::<HashMap<&'static str, &'static str>>(),
        );

      let registry = match self.namespaced_registry.get(namespace).unwrap() {
        Ok(reg) => reg,
        Err(Error::MissingVariants(names)) => {
          self.errors.push(format!(r#"
##
#
# An enum was used that has not had the membrane::dart_enum macro applied for a namespace which owns or borrows it.
#
# Please add #[dart_enum(namespace = "{}")] to the {} enum.
#
##"#,
            namespace,
            names.first().unwrap()
          ));

          return self;
        }
        Err(err) => {
          self.errors.push(err.to_string());
          return self;
        }
      };

      if let Err(e) = installer.install_module(&config, registry) {
        self
          .errors
          .push(format!("unable to install module {namespace}: {e}"));
        return self;
      }
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
      .expect("failed to spawn dart process");

    if pub_get.status.code() != Some(0) {
      let _ = std::io::stderr().write_all(&pub_get.stderr);
      let _ = std::io::stdout().write_all(&pub_get.stdout);
      self
        .errors
        .push("'dart pub get' returned an error".to_string());
    }

    self
  }

  ///
  /// When set to `true` (the default) we generate basic Dart enums. When set to `false`
  /// Dart classes are generated (one for the base case and one for each variant).
  pub fn with_c_style_enums(&mut self, val: bool) -> &mut Self {
    return_if_error!(self);
    self.c_style_enums = val;
    self
  }

  ///
  /// When set to `true` (the default) we generate sealed classes for complex enums
  /// instead of abstract classes. When set to `false`
  /// abstract classes are generated.
  pub fn with_sealed_enums(&mut self, val: bool) -> &mut Self {
    return_if_error!(self);
    self.sealed_enums = val;
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
    return_if_error!(self);
    self.timeout = Some(val);
    self
  }

  ///
  /// Configures some aspects of the generated Dart code.
  ///
  /// Default:
  ///
  /// DartConfig {
  ///   logger: DartLoggerConfig {
  ///     import_path: "package:logging/logging.dart",
  ///     instance: "Logger('membrane')",
  ///     info_log_fn: "info",
  ///     fine_log_fn: "fine",
  ///   }
  /// }
  pub fn dart_config(&mut self, config: DartConfig) -> &mut Self {
    return_if_error!(self);
    self.dart_config = config;
    self
  }

  ///
  /// Write a header file for each namespace that provides the C types
  /// needed by ffigen to generate the FFI bindings.
  pub fn write_c_headers(&mut self) -> &mut Self {
    return_if_error!(self);
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
char * membrane_metadata_version();
uint8_t membrane_free_membrane_string(char *ptr);

#endif
"#;

    let path = self.destination.join("lib/src/membrane_types.h");
    std::fs::write(&path, head).unwrap_or_else(|_| {
      self
        .errors
        .push(format!("unable to write {}", path.to_str().unwrap()));
    });

    let namespaces = std::mem::take(&mut self.namespaces);
    for x in &namespaces {
      self.write_header(x);
    }
    self.namespaces = namespaces;

    self
  }

  ///
  /// Write all Dart classes needed by the Dart application.
  pub fn write_api(&mut self) -> &mut Self {
    return_if_error!(self);
    let namespaces = std::mem::take(&mut self.namespaces);
    for x in &namespaces {
      self.create_ffi_impl(x);
      self.create_web_impl(x);
      self.create_class(x);
    }
    self.namespaces = namespaces;

    self.create_imports();

    if self.generated {
      self.create_loader();
      self.create_exceptions();
      self.format_package();
    }

    self
  }

  ///
  /// Invokes `dart run ffigen` with the appropriate config to generate FFI bindings.
  pub fn write_bindings(&mut self) -> &mut Self {
    return_if_error!(self);
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
      .expect("failed to spawn dart process");

    if ffigen.status.code() != Some(0) {
      let _ = std::io::stderr().write_all(&ffigen.stderr);
      let _ = std::io::stdout().write_all(&ffigen.stdout);
      self
        .errors
        .push("dart ffigen returned an error".to_string());
    }

    self
  }

  ///
  /// Whether any errors were found during code generation.
  pub fn is_err(&mut self) -> bool {
    !self.errors.is_empty()
  }

  ///
  /// Returns all codegen errors and empties the error queue. This will prevent Membrane from
  /// automatically exiting `1` and allow you to implement your own CLI exit handling if needed.
  pub fn drain_errors(&mut self) -> Vec<String> {
    self.errors.drain(..).collect()
  }

  ///
  /// Private implementations
  ///
  fn write_pubspec(&mut self) -> &mut Self {
    // serde-generate uses the last namespace as the pubspec name and dart doesn't
    // like that so we set a proper package name from the basename or from an explicitly given name
    if self.package_name.is_empty() {
      self.package_name = self
        .destination
        .file_name()
        .expect("destination path must not be empty or end in '..'")
        .to_str()
        .expect("destination path must be valid UTF-8")
        .to_string()
    }
    let path = self.destination.join("pubspec.yaml");

    if let Ok(old) = std::fs::read_to_string(&path) {
      let pubspec = old
        .lines()
        .filter(|l| !l.is_empty())
        .map(|ln| {
          if ln.contains("name:") {
            format!("name: {}", self.package_name)
          } else if ln.contains("sdk:") {
            // ffigen >= 5 requires dart >= 2.17, so replace dart version from serde-reflection
            format!("  sdk: '{}'", self.dart_config.versions["sdk"])
          } else {
            ln.to_owned()
          }
        })
        .chain(vec![
          format!("  ffi: {}", self.dart_config.versions["ffi"]),
          format!(
            "  {}: {}",
            self.dart_config.logger.dependency_name, self.dart_config.versions["logger"]
          ),
          "dev_dependencies:".to_owned(),
          format!("  ffigen: {}\n", self.dart_config.versions["ffigen"]),
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
      if self.llvm_paths.is_empty() {
        String::new()
      } else {
        "llvm-path:".to_string()
          + &self
            .llvm_paths
            .iter()
            .map(|p| "\n  - '".to_string() + p + "'")
            .collect::<String>()
      }
    );

    let path = self.destination.join("ffigen.yaml");
    std::fs::write(&path, config).unwrap_or_else(|_| {
      self.errors.push(format!(
        "unable to write ffigen config {}",
        path.to_str().unwrap()
      ));
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
      std::fs::File::create(&path).expect("header could not be written at namespace path");

    if buffer.write_all(head.as_bytes()).is_err() {
      self.errors.push(format!(
        "unable to write C header file {}",
        path.to_str().unwrap()
      ));
    }

    for x in fns {
      generators::functions::C::new(x).build(self).write(&buffer);
    }

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

  fn create_exceptions(&mut self) -> &mut Self {
    let helpers = exceptions::create_exceptions();
    let path = self.destination.join("lib/src/membrane_exceptions.dart");
    std::fs::write(path, helpers).expect("membrane_exceptions.dart could not be written");

    let barrel_exceptions = "// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
export './src/membrane_exceptions.dart';";

    let path = self.destination.join("lib/membrane_exceptions.dart");
    std::fs::write(path, barrel_exceptions)
      .expect("membrane_exceptions.dart barrel could not be written");

    self
  }

  fn create_loader(&mut self) -> &mut Self {
    let ffi_loader = loaders::create_ffi_loader(&self.library, &self.dart_config);
    let path = self.destination.join("lib/src/membrane_loader_ffi.dart");
    std::fs::write(path, ffi_loader).expect("membrane_loader_ffi.dart could not be written");

    let web_loader = loaders::create_web_loader(&self.library);
    let path = self.destination.join("lib/src/membrane_loader_web.dart");
    std::fs::write(path, web_loader).expect("membrane_loader_web.dart could not be written");

    let barrel_loader = "// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
export './membrane_loader_ffi.dart' if (dart.library.html) './membrane_loader_web.dart';";

    let path = self.destination.join("lib/src/membrane_loader.dart");
    std::fs::write(path, barrel_loader).expect("membrane_loader.dart could not be written");

    self
  }

  fn create_class(&mut self, namespace: &str) -> &mut Self {
    use std::io::prelude::*;
    let path = self
      .destination
      .join("lib")
      .join(format!("{namespace}.dart"));

    let head = if utils::new_style_export(namespace, &self.dart_config) {
      format!(
        r#"// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
export './src/{ns}_ffi.dart' if (dart.library.html) './src/{ns}_web.dart';

export './src/{ns}/{ns}.dart' hide TraitHelpers;
"#,
        ns = &namespace,
      )
    } else {
      format!(
        r#"// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
export './src/{ns}_ffi.dart' if (dart.library.html) './src/{ns}_web.dart';
"#,
        ns = &namespace,
      )
    };

    let mut buffer = std::fs::File::create(path).expect("class could not be written at path");
    buffer
      .write_all(head.as_bytes())
      .expect("failed to write generated Dart file");

    self
  }

  fn create_ffi_impl(&mut self, namespace: &str) -> &mut Self {
    use std::io::prelude::*;
    let path = self
      .destination
      .join("lib/src")
      .join(namespace.to_string() + "_ffi.dart");

    if !self.namespaced_fn_registry.contains_key(namespace) {
      let head = format!(
        "export './{ns}/{ns}.dart' hide TraitHelpers;",
        ns = &namespace
      );
      let mut buffer = std::fs::File::create(path).expect("class could not be written at path");
      buffer
        .write_all(head.as_bytes())
        .expect("failed to write generated Dart file");

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
import '{logger_path}';
import 'package:meta/meta.dart';

import './membrane_exceptions.dart';
import './membrane_loader.dart' as loader;
import './bincode/bincode.dart';
import './ffi_bindings.dart' show MembraneMsgKind, MembraneResponse, MembraneResponseKind;
import './{ns}/{ns}.dart';
{export}
final _bindings = loader.bindings;
final _loggingDisabled = bool.fromEnvironment('MEMBRANE_DISABLE_LOGS');

@immutable
class {class_name}ApiError implements Exception {{
  final e;
  const {class_name}ApiError(this.e);

  @override
  String toString() {{
    return (e == null)
        ? "{class_name}ApiError"
        : "{class_name}ApiError: $e";
  }}
}}

@immutable
class {class_name}Api {{
  static final _log = {logger};
  const {class_name}Api();
"#,
      ns = &namespace,
      class_name = &namespace.to_upper_camel_case(),
      logger_path = self.dart_config.logger.import_path,
      logger = self
        .dart_config
        .logger
        .instance
        .replace("')", &format!(".{}')", &namespace))
        .replace("\")", &format!(".{}\")", &namespace)),
      export = if utils::new_style_export(namespace, &self.dart_config) {
        String::new()
      } else {
        format!(
          "\nexport './{ns}/{ns}.dart' hide TraitHelpers;\n",
          ns = &namespace
        )
      }
    );

    let mut buffer = std::fs::File::create(path).expect("class could not be written at path");
    buffer
      .write_all(head.as_bytes())
      .expect("failed to write generated Dart file");

    for x in fns {
      generators::functions::Ffi::new(x)
        .build(self)
        .write(&buffer);
    }

    buffer
      .write_all(b"}\n")
      .expect("failed to write generated Dart file");

    self
  }

  fn create_web_impl(&mut self, namespace: &str) -> &mut Self {
    use std::io::prelude::*;
    let path = self
      .destination
      .join("lib/src")
      .join(namespace.to_string() + "_web.dart");

    // perhaps this namespace has only enums in it and no functions
    if !self.namespaced_fn_registry.contains_key(namespace) {
      let head = if utils::new_style_export(namespace, &self.dart_config) {
        String::new()
      } else {
        format!(
          "export './{ns}/{ns}.dart' hide TraitHelpers;",
          ns = &namespace
        )
      };
      let mut buffer = std::fs::File::create(path).expect("class could not be written at path");
      buffer
        .write_all(head.as_bytes())
        .expect("failed to write generated Dart file");

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
import './membrane_exceptions.dart';
import './{ns}/{ns}.dart';
{export}
@immutable
class {class_name}ApiError implements Exception {{
  final e;
  const {class_name}ApiError(this.e);

  @override
  String toString() {{
    return (e == null)
        ? "{class_name}ApiError"
        : "{class_name}ApiError: $e";
  }}
}}

@immutable
class {class_name}Api {{
  const {class_name}Api();
"#,
      ns = &namespace,
      class_name = &namespace.to_upper_camel_case(),
      export = if utils::new_style_export(namespace, &self.dart_config) {
        String::new()
      } else {
        format!("export './{namespace}/{namespace}.dart' hide TraitHelpers;")
      }
    );

    let mut buffer = std::fs::File::create(path).expect("class could not be written at path");
    buffer
      .write_all(head.as_bytes())
      .expect("failed to write generated Dart file");

    for x in fns {
      generators::functions::Web::new(x)
        .build(self)
        .write(&buffer);
    }

    buffer
      .write_all(b"}\n")
      .expect("failed to write generated Dart file");

    self
  }

  fn namespace_path(&mut self, namespace: &str) -> PathBuf {
    self.destination.join("lib").join("src").join(namespace)
  }

  fn create_imports(&mut self) -> &mut Self {
    let mut owned_types: Vec<String> = vec![];
    let mut non_owned_types: Vec<String> = vec![];
    let borrows = std::mem::take(&mut self.borrows);

    for (namespace, imports) in &borrows {
      for (from_namespace, (borrowed_types, borrow_locations_for_type)) in imports.iter().rev() {
        let mut borrowed_types: Vec<String> = borrowed_types.iter().flat_map(|r#type| {
            if namespace == from_namespace {
              self.errors.push(format!("`{ns}::{import}`{location_hint} was borrowed by `{ns}` which is a self reference", location_hint = utils::display_code_location(borrow_locations_for_type.get(r#type)), ns = namespace, import = r#type));
            }

            let auto_import = self.with_child_borrows(from_namespace, r#type);
            for x in &auto_import {
              if borrowed_types.contains(x.as_str()) && x != r#type {
                warn!("{ns}::{import} was explicitly borrowed{manual_hint} but it is already implicitly borrowed because it is a subtype of `{ns}::{type}`{auto_hint}. Remove the `{ns}::{import}` borrow.",
                ns = from_namespace, manual_hint = utils::display_code_location(borrow_locations_for_type.get(x.as_str())), import = x, r#type = r#type, auto_hint = utils::display_code_location(borrow_locations_for_type.get(r#type)));
              }
            }

            auto_import
          }).collect();

        borrowed_types.sort();
        borrowed_types.dedup();

        // this is the path that is owned (IE the namespace that holds the Rust source type)
        owned_types.extend(borrowed_types.iter().map(|ty| format!("{namespace}::{ty}")));
        // and this is the borrowed path
        non_owned_types.extend(
          borrowed_types
            .iter()
            .map(|ty| format!("{from_namespace}::{ty}")),
        );

        let src_path = self.destination.join("lib/src");
        let namespace_path = src_path.join(namespace);

        imports::inject_imports(namespace_path.join(format!("{namespace}.dart")), |line| {
          // because CamelCasing the snake_cased `part 'central_usa.dart'` won't match the
          // acronym borrow `CentralUSA` we instead convert the borrows to snake_case to do the match
          if borrowed_types
            .iter()
            .map(|t| t.to_snake_case())
            .collect::<Vec<String>>()
            .contains(&line.replace("part '", "").replace(".dart';", ""))
          {
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
          } else if line.starts_with("export '../serde")
            && !utils::new_style_export(namespace, &self.dart_config)
          {
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
        .expect("failed to inject imports into generated Dart file");

        if utils::new_style_export(namespace, &self.dart_config) {
          imports::inject_imports(src_path.join(format!("{namespace}_ffi.dart")), |line| {
            if line.starts_with(&format!("import './{namespace}/{namespace}.dart'")) {
              Some(vec![
                line.to_string(),
                format!(
                  "import './{from_namespace}/{from_namespace}.dart' show {types};",
                  types = borrowed_types.join(",")
                ),
              ])
            } else {
              Some(vec![line.to_string()])
            }
          })
          .expect("failed to inject imports into generated Dart FFI file");

          imports::inject_imports(src_path.join(format!("{namespace}_web.dart")), |line| {
            if line.starts_with(&format!("import './{namespace}/{namespace}.dart'")) {
              Some(vec![
                line.to_string(),
                format!(
                  "import './{from_namespace}/{from_namespace}.dart' show {types};",
                  types = borrowed_types.join(",")
                ),
              ])
            } else {
              Some(vec![line.to_string()])
            }
          })
          .expect("failed to inject imports into generated Dart web file");
        }

        for borrowed_type in &borrowed_types {
          let filename = format!("{}.dart", borrowed_type.to_snake_case());
          let _ = remove_file(namespace_path.join(filename));
        }
      }
    }
    self.borrows = borrows;

    // if we already have an error about borrows above then lets exit with that
    return_if_error!(self);

    let mut reborrows: Vec<String> = non_owned_types
      .iter()
      .filter(|path| owned_types.contains(path))
      .cloned()
      .collect();

    reborrows.sort();

    if !reborrows.is_empty() {
      self.errors.push(format!("The following `borrows` were found which attempt to reborrow a type which is not owned by the target namespace: `{}`", reborrows.join(", ")));
    }

    self
  }
}

impl Drop for Membrane {
  fn drop(&mut self) {
    if self.is_err() {
      for err in &self.errors {
        tracing::error!("{}", err);
      }
      exit(1);
    }
  }
}

#[doc(hidden)]
pub use ffi_types::{
  membrane_cancel_membrane_task, membrane_free_membrane_string, membrane_free_membrane_vec,
  MembraneMsgKind, MembraneResponse, MembraneResponseKind, TaskHandle,
};

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
