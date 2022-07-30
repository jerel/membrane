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
pub mod utils;

use membrane_types::dart::dart_type;
use membrane_types::heck::CamelCase;
use serde_reflection::{ContainerFormat, Error, Registry, Samples, Tracer, TracerConfig};
use std::{
  collections::HashMap,
  io::Write,
  path::{Path, PathBuf},
};

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct Function {
  pub extern_c_fn_name: String,
  pub extern_c_fn_types: String,
  pub fn_name: String,
  pub is_stream: bool,
  pub is_sync: bool,
  pub return_type: Vec<&'static str>,
  pub error_type: Vec<&'static str>,
  pub namespace: String,
  pub disable_logging: bool,
  pub timeout: Option<i32>,
  pub output: String,
  pub dart_outer_params: String,
  pub dart_transforms: String,
  pub dart_inner_args: String,
}

#[doc(hidden)]
pub struct DeferredTrace {
  pub function: Function,
  pub namespace: String,
  pub trace: fn(tracer: &mut serde_reflection::Tracer, samples: &mut serde_reflection::Samples),
}

#[doc(hidden)]
pub struct DeferredEnumTrace {
  pub namespace: String,
  pub trace: fn(tracer: &mut serde_reflection::Tracer),
}

inventory::collect!(DeferredTrace);
inventory::collect!(DeferredEnumTrace);

pub struct Membrane {
  package_name: String,
  destination: PathBuf,
  library: String,
  llvm_paths: Vec<String>,
  namespaces: Vec<String>,
  namespaced_enum_registry: HashMap<String, serde_reflection::Result<Registry>>,
  namespaced_fn_registry: HashMap<String, Vec<Function>>,
  generated: bool,
  c_style_enums: bool,
  timeout: Option<i32>,
}

impl<'a> Membrane {
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let mut namespaces = vec![];
    let mut namespaced_enum_registry = HashMap::new();
    let mut namespaced_samples = HashMap::new();
    let mut namespaced_fn_registry = HashMap::new();
    for item in inventory::iter::<DeferredEnumTrace> {
      namespaces.push(item.namespace.clone());

      let tracer = namespaced_enum_registry
        .entry(item.namespace.clone())
        .or_insert_with(|| Tracer::new(TracerConfig::default()));

      (item.trace)(tracer);
    }

    for item in inventory::iter::<DeferredTrace> {
      namespaces.push(item.namespace.clone());

      let tracer = namespaced_enum_registry
        .entry(item.namespace.clone())
        .or_insert_with(|| Tracer::new(TracerConfig::default()));

      let samples = namespaced_samples
        .entry(item.namespace.clone())
        .or_insert_with(Samples::new);

      (item.trace)(tracer, samples);

      namespaced_fn_registry
        .entry(item.namespace.clone())
        .or_insert_with(Vec::new)
        .push(item.function.clone());
    }

    namespaces.sort();
    namespaces.dedup();

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
      namespaced_enum_registry: namespaced_enum_registry
        .into_iter()
        .map(|(key, val)| (key, val.registry()))
        .collect(),
      namespaced_fn_registry,
      namespaces,
      generated: false,
      c_style_enums: true,
      timeout: None,
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
      let config = serde_generate::CodeGeneratorConfig::new(namespace.to_string())
        .with_encodings(vec![serde_generate::Encoding::Bincode])
        .with_c_style_enums(self.c_style_enums);

      let registry = match self.namespaced_enum_registry.get(namespace).unwrap() {
        Ok(reg) => reg,
        Err(Error::MissingVariants(names)) => {
          panic!(
            "An enum was used that has not had the membrane::dart_enum macro applied. Please add #[dart_enum(namespace = \"{}\")] to the {} enum.",
            namespace,
            names.first().unwrap()
          );
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
      panic!("'dart pub get' returned an error");
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
      panic!("unable to write {}", path.to_str().unwrap());
    });

    let namespaces = self.namespaces.clone();
    namespaces.iter().for_each(|x| {
      self.write_header(x.to_string());
    });

    self
  }

  ///
  /// Write all Dart classes needed by the Dart application.
  pub fn write_api(&mut self) -> &mut Self {
    let namespaces = self.namespaces.clone();
    namespaces.iter().for_each(|x| {
      self.create_class(x.to_string());
    });

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
      panic!("dart ffigen returned an error");
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
        .chain([
          "  logging: ^1.0.2\n".to_owned(),
          "dev_dependencies:".to_owned(),
          "  ffigen: ^6.0.1\n".to_owned(),
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
      panic!("unable to write ffigen config {}", path.to_str().unwrap());
    });

    self
  }

  fn write_header(&mut self, namespace: String) -> &mut Self {
    use std::io::prelude::*;
    let path = self
      .namespace_path(namespace.clone())
      .join(namespace.to_string() + ".h");
    let fns = self.namespaced_fn_registry.get(&namespace).unwrap();

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
      panic!("unable to write C header file {}", path.to_str().unwrap());
    });

    fns.iter().for_each(|x| {
      let mut fun = x.clone();
      fun.begin().c_signature().write(&buffer);
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
    let base_class = format!(
      r#"// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
import 'dart:ffi';
import 'dart:io' show Platform;
import 'package:logging/logging.dart';

import './ffi_bindings.dart' as ffi_bindings;

DynamicLibrary _open() {{
  if (Platform.isLinux) {{
    Logger('membrane').info('Opening native library {lib}.so');
    return DynamicLibrary.open('{lib}.so');
  }}
  if (Platform.isAndroid) {{
    Logger('membrane').info('Opening native library {lib}.so');
    return DynamicLibrary.open('{lib}.so');
  }}
  if (Platform.isIOS) {{
    Logger('membrane').info('Creating dynamic library {lib}');
    return DynamicLibrary.executable();
  }}
  if (Platform.isMacOS) {{
    Logger('membrane').info('Opening native library {lib}.dylib');
    return DynamicLibrary.open('{lib}.dylib');
  }}
  if (Platform.isWindows) {{
    Logger('membrane').info('Opening native library {lib}.dll');
    return DynamicLibrary.open('{lib}.dll');
  }}
  throw UnsupportedError('This platform is not supported.');
}}

typedef _StoreDartPostCobjectC = Void Function(
  Pointer<NativeFunction<Int8 Function(Int64, Pointer<Dart_CObject>)>> ptr,
);
typedef _StoreDartPostCobjectDart = void Function(
  Pointer<NativeFunction<Int8 Function(Int64, Pointer<Dart_CObject>)>> ptr,
);

_load() {{
  final dl = _open();
  Logger('membrane').info('Initializing FFI bindings');
  final bindings = ffi_bindings.NativeLibrary(dl);
  final storeDartPostCobject =
      dl.lookupFunction<_StoreDartPostCobjectC, _StoreDartPostCobjectDart>(
    'store_dart_post_cobject',
  );

  Logger('membrane').fine('Initializing Dart_PostCObject');
  storeDartPostCobject(NativeApi.postCObject);

  bindingsLoaded = true;
  return bindings;
}}

// Prefer using `bindings` without checking `bindingsLoaded` for most cases.
// This boolean is for special cases where the Dart application needs to
// perform differently until another part of the application needs to load
// the bindings. For example if debug logs are sent to Rust via FFI then you
// may want to log locally in Dart until bindings are loaded and at that time
// begin sending logs over the FFI boundary.
bool bindingsLoaded = false;

final bindings = _load();
"#,
      lib = self.library,
    );

    let path = self.destination.join("lib").join("src").join("loader.dart");
    std::fs::write(path, base_class).unwrap();

    self
  }

  fn create_class(&mut self, namespace: String) -> &mut Self {
    use std::io::prelude::*;
    let path = self
      .destination
      .join("lib")
      .join(namespace.to_string() + ".dart");

    // perhaps this namespace has only enums in it and no functions
    if self.namespaced_fn_registry.get(&namespace).is_none() {
      return self;
    }

    let fns = self.namespaced_fn_registry.get(&namespace).unwrap();
    let enum_registry = self
      .namespaced_enum_registry
      .get(&namespace)
      .unwrap()
      // we've already inspected the registry for incomplete enums, now we'll have only valid ones
      .as_ref().unwrap();

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

import './src/loader.dart' as loader;
import './src/bincode/bincode.dart';
import './src/ffi_bindings.dart' show MembraneMsgKind, MembraneResponse, MembraneResponseKind;
import './src/{ns}/{ns}.dart';

export './src/{ns}/{ns}.dart' hide TraitHelpers;

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
      let mut fun = x.clone();
      fun
        .begin()
        .signature()
        .body(&namespace)
        .body_return(&namespace, enum_registry, self)
        .end()
        .write(&buffer);
    });

    buffer.write_all(b"}\n").unwrap();

    self
  }

  fn namespace_path(&mut self, namespace: String) -> PathBuf {
    self.destination.join("lib").join("src").join(&namespace)
  }
}

impl Function {
  pub fn begin(&mut self) -> &mut Self {
    self.output = "\n".to_string();
    self
  }

  pub fn signature(&mut self) -> &mut Self {
    self.output += format!(
      "  {output_style}{return_type} {fn_name}({fn_params}){asink}",
      output_style = if self.is_sync {
        ""
      } else if self.is_stream {
        "Stream"
      } else {
        "Future"
      },
      return_type = if self.is_sync {
        dart_type(&self.return_type)
      } else {
        format!("<{}>", dart_type(&self.return_type))
      },
      fn_name = self.fn_name,
      fn_params = if self.dart_outer_params.is_empty() {
        String::new()
      } else {
        format!("{{{}}}", self.dart_outer_params)
      },
      asink = if self.is_sync {
        ""
      } else if self.is_stream {
        " async*"
      } else {
        " async"
      }
    )
    .as_str();
    self
  }
  pub fn c_signature(&mut self) -> &mut Self {
    self.output += format!(
      "MembraneResponse {extern_c_fn_name}({port}{extern_c_fn_types});",
      extern_c_fn_name = self.extern_c_fn_name,
      port = if self.is_sync { "" } else { "int64_t port" },
      extern_c_fn_types = if self.extern_c_fn_types.is_empty() {
        String::new()
      } else if self.is_sync {
        String::new() + &self.extern_c_fn_types
      } else {
        String::from(", ") + &self.extern_c_fn_types
      }
    )
    .as_str();
    self
  }

  pub fn body(&mut self, namespace: &str) -> &mut Self {
    self.output += format!(
      r#" {{{disable_logging}
    final List<Pointer> _toFree = [];{fn_transforms}{receive_port}

    MembraneResponse _taskResult;
    try {{
      if (!_loggingDisabled) {{
        _log.fine('Calling Rust `{fn_name}` via C `{extern_c_fn_name}`');
      }}
      _taskResult = _bindings.{extern_c_fn_name}({native_port}{dart_inner_args});
      if (_taskResult.kind == MembraneResponseKind.panic) {{
        final ptr = _taskResult.data.cast<Utf8>();
        throw {class_name}ApiError(ptr.toDartString());
      }} else if (_taskResult.kind != MembraneResponseKind.data) {{
        throw {class_name}ApiError('Found unknown MembraneResponseKind variant, mismatched code versions?');
      }}
    }} finally {{
      _toFree.forEach((ptr) => calloc.free(ptr));
      if (!_loggingDisabled) {{
        _log.fine('Freed arguments to `{extern_c_fn_name}`');
      }}
    }}
"#,
      disable_logging = if self.disable_logging {
        "final _loggingDisabled = true;"
      } else {
        ""
      },
      fn_transforms = if self.dart_transforms.is_empty() {
        String::new()
      } else {
        "\n    ".to_string() + &self.dart_transforms + ";"
      },
      receive_port = if self.is_sync {
        ""
      } else {
        "\n    final _port = ReceivePort();"
      },
      extern_c_fn_name = self.extern_c_fn_name,
      fn_name = self.fn_name,
      native_port = if self.is_sync {
        ""
      } else {
        "_port.sendPort.nativePort"
      },
      dart_inner_args = if self.dart_inner_args.is_empty() {
        String::new()
      } else if self.is_sync {
        String::new() + &self.dart_inner_args
      } else {
        String::from(", ") + &self.dart_inner_args
      },
      class_name = namespace.to_camel_case()
    )
    .as_str();
    self
  }

  pub fn body_return(
    &mut self,
    namespace: &str,
    enum_tracer_registry: &Registry,
    config: &Membrane,
  ) -> &mut Self {
    self.output += if self.is_sync {
      format!(
        r#"
    final data = _taskResult.data.cast<Uint8>();
    final length = ByteData.view(data.asTypedList(8).buffer).getInt64(0, Endian.little);
    try {{
      if (!_loggingDisabled) {{
        _log.fine('Deserializing data from {fn_name}');
      }}
      final deserializer = BincodeDeserializer(data.asTypedList(length + 8).sublist(8));
      if (deserializer.deserializeUint8() == MembraneMsgKind.ok) {{
        return {return_de};
      }}
      throw {class_name}ApiError({error_de});
    }} finally {{
      if (_taskResult.kind == MembraneResponseKind.data && _bindings.membrane_free_membrane_vec(length + 8, _taskResult.data) < 1) {{
        throw {class_name}ApiError('Resource freeing call to C failed');
      }}
    }}"#,
        return_de = self.deserializer(&self.return_type, enum_tracer_registry, config),
        error_de = self.deserializer(&self.error_type, enum_tracer_registry, config),
        class_name = namespace.to_camel_case(),
        fn_name = self.fn_name,
      )
    } else if self.is_stream {
      format!(
        r#"
    try {{
      yield* _port{timeout}.map((input) {{
        if (!_loggingDisabled) {{
          _log.fine('Deserializing data from {fn_name}');
        }}
        final deserializer = BincodeDeserializer(input as Uint8List);
        if (deserializer.deserializeUint8() == MembraneMsgKind.ok) {{
          return {return_de};
        }}
        throw {class_name}ApiError({error_de});
      }});
    }} finally {{
      if (_taskResult.kind == MembraneResponseKind.data && _bindings.membrane_cancel_membrane_task(_taskResult.data) < 1) {{
        throw {class_name}ApiError('Cancellation call to C failed');
      }}
    }}"#,
        return_de = self.deserializer(&self.return_type, enum_tracer_registry, config),
        error_de = self.deserializer(&self.error_type, enum_tracer_registry, config),
        class_name = namespace.to_camel_case(),
        fn_name = self.fn_name,
        timeout = if let Some(val) = self.timeout {
          // check the async_dart option configured timeout
          format!(".timeout(const Duration(milliseconds: {}))", val)
        } else {
          // we default to no timeout even if a global timeout is configured because
          // having all streams auto-disconnect after a pause in events is not desirable
          "".to_string()
        },
      )
    } else {
      format!(
        r#"
    try {{
      if (!_loggingDisabled) {{
        _log.fine('Deserializing data from {fn_name}');
      }}
      final deserializer = BincodeDeserializer(await _port.first{timeout} as Uint8List);
      if (deserializer.deserializeUint8() == MembraneMsgKind.ok) {{
        return {return_de};
      }}
      throw {class_name}ApiError({error_de});
    }} finally {{
      if (_taskResult.kind == MembraneResponseKind.data && _bindings.membrane_cancel_membrane_task(_taskResult.data) < 1) {{
        throw {class_name}ApiError('Cancellation call to C failed');
      }}
    }}"#,
        return_de = self.deserializer(&self.return_type, enum_tracer_registry, config),
        error_de = self.deserializer(&self.error_type, enum_tracer_registry, config),
        class_name = namespace.to_camel_case(),
        fn_name = self.fn_name,
        timeout = if let Some(val) = self.timeout {
          // check the async_dart option configured timeout first
          format!(".timeout(const Duration(milliseconds: {}))", val)
        } else if let Some(val) = config.timeout {
          // fall back to global timeout
          format!(".timeout(const Duration(milliseconds: {}))", val)
        } else {
          // and by default we won't time out at all
          "".to_string()
        },
      )
    }
    .as_str();

    self
  }

  pub fn end(&mut self) -> &mut Self {
    self.output += "\n  }\n";
    self
  }

  pub fn write(&mut self, mut buffer: &std::fs::File) -> &mut Self {
    buffer
      .write_all(self.output.as_bytes())
      .expect("function could not be written at path");
    self
  }

  fn deserializer(
    &self,
    ty: &[&str],
    enum_tracer_registry: &Registry,
    config: &Membrane,
  ) -> String {
    let de;
    match ty[..] {
      ["String"] => "deserializer.deserializeString()",
      ["i8"] => "deserializer.deserializeInt8()",
      ["u8"] => "deserializer.deserializeUint8()",
      ["i16"] => "deserializer.deserializeInt16()",
      ["u16"] => "deserializer.deserializeUint16()",
      ["i32"] => "deserializer.deserializeInt32()",
      ["u32"] => "deserializer.deserializeUint32()",
      ["i64"] => "deserializer.deserializeInt64()",
      ["f32"] => "deserializer.deserializeFloat32()",
      ["f64"] => "deserializer.deserializeFloat64()",
      ["bool"] => "deserializer.deserializeBool()",
      ["()"] => "null",
      ["Vec", "Option", ..] => {
        de = format!(
          "List.generate(deserializer.deserializeLength(), (_i) {{
            if (deserializer.deserializeOptionTag()) {{
              return {};
            }}
            return null;
          }});",
          self.deserializer(&ty[2..], enum_tracer_registry, config)
        );
        &de
      }
      ["Vec", ..] => {
        de = format!(
          "List.generate(deserializer.deserializeLength(), (_i) {{
            return {};
          }});",
          self.deserializer(&ty[1..], enum_tracer_registry, config)
        );
        &de
      }
      ["Option", _ty] => {
        panic!(
          "Option is not supported as a bare return type. Return the inner type from {} instead",
          self.fn_name
        )
      }
      [ty, ..] => {
        de = match enum_tracer_registry.get(ty) {
          Some(ContainerFormat::Enum { .. }) if config.c_style_enums => {
            format!("{}Extension.deserialize(deserializer)", ty)
          }
          _ => format!("{}.deserialize(deserializer)", ty),
        };
        &de
      }
      [] => {
        unreachable!("Expected type information to exist")
      }
    }
    .to_string()
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
