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
pub use membrane_macro::{async_dart, dart_enum};
#[doc(hidden)]
pub use serde_reflection;

use heck::CamelCase;
use membrane_types::dart::dart_fn_return_type;
use serde_reflection::{Error, Samples, Tracer, TracerConfig};
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
  pub return_type: String,
  pub error_type: String,
  pub namespace: String,
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
  namespaced_registry: HashMap<String, Tracer>,
  namespaced_fn_registry: HashMap<String, Vec<Function>>,
  generated: bool,
  c_style_enums: bool,
}

impl<'a> Membrane {
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let mut namespaces = vec![];
    let mut namespaced_registry = HashMap::new();
    let mut namespaced_samples = HashMap::new();
    let mut namespaced_fn_registry = HashMap::new();
    for item in inventory::iter::<DeferredEnumTrace> {
      let entry = namespaced_registry
        .entry(item.namespace.clone())
        .or_insert_with(|| Tracer::new(TracerConfig::default()));

      (item.trace)(entry);
    }

    for item in inventory::iter::<DeferredTrace> {
      namespaces.push(item.namespace.clone());

      let entry = namespaced_registry
        .entry(item.namespace.clone())
        .or_insert_with(|| Tracer::new(TracerConfig::default()));

      let samples = namespaced_samples
        .entry(item.namespace.clone())
        .or_insert_with(Samples::new);

      (item.trace)(entry, samples);

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
      namespaced_registry,
      namespaced_fn_registry,
      namespaces,
      generated: false,
      c_style_enums: true,
    }
  }

  ///
  /// The directory for the pub package output. The basename will be the name of the pub package.
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
  /// The name of the generate package.
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
  /// The name of the dylib or so that the Rust project produces. Membrane
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

      let tracer = self.namespaced_registry.remove(namespace).unwrap();
      let registry = match tracer.registry() {
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
        .output(self.destination.to_path_buf(), &registry)
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
  /// Write a header file for each namespace that provides the C types
  /// needed by ffigen to generate the FFI bindings.
  pub fn write_c_headers(&mut self) -> &mut Self {
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
    let re = regex::Regex::new(r"^name:(.*?)\n").unwrap();
    if let Ok(old) = std::fs::read_to_string(&path) {
      let pubspec = re
        .replace(
          old.as_str().trim(),
          "name: ".to_string() + &package_name + "\n",
        )
        .to_string();

      let extra_deps = r#"
  ffi: ^1.1.2

dev_dependencies:
  ffigen: ^4.1.0
"#;
      std::fs::write(path, pubspec + extra_deps).expect("pubspec could not be written");
    }

    self
  }

  fn write_ffigen_config(&mut self) -> &mut Self {
    let config = format!(
      r#"name: 'NativeLibrary'
description: 'Auto generated bindings for Dart types'
output: './lib/src/ffi_bindings.dart'
headers:
  entry-points:
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

typedef int TaskHandle;

int32_t membrane_cancel_membrane_task(const TaskHandle *task_handle);
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

import './ffi_bindings.dart' as ffi_bindings;

DynamicLibrary _open() {{
  if (Platform.isLinux) return DynamicLibrary.open('{lib}.so');
  if (Platform.isAndroid) return DynamicLibrary.open('{lib}.so');
  if (Platform.isIOS) return DynamicLibrary.executable();
  if (Platform.isMacOS) return DynamicLibrary.open('{lib}.dylib');
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
  final bindings = ffi_bindings.NativeLibrary(dl);
  final storeDartPostCobject =
      dl.lookupFunction<_StoreDartPostCobjectC, _StoreDartPostCobjectDart>(
    'store_dart_post_cobject',
  );

  storeDartPostCobject(NativeApi.postCObject);

  return bindings;
}}

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
    let fns = self.namespaced_fn_registry.get(&namespace).unwrap();

    let head = format!(
      r#"// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
import 'dart:ffi';
import 'dart:isolate' show ReceivePort;
import 'dart:typed_data';
import 'package:ffi/ffi.dart';
import 'package:meta/meta.dart';

import './src/loader.dart' as loader;
import './src/bincode/bincode.dart';
import './src/{ns}/{ns}.dart';

export './src/{ns}/{ns}.dart' hide TraitHelpers;

final _bindings = loader.bindings;

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
      let mut fun = x.clone();
      fun
        .begin()
        .signature()
        .body(&namespace)
        .body_return(&namespace)
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
      "  {output_style}<{return_type}> {fn_name}({fn_params}){asink}",
      output_style = if self.is_stream { "Stream" } else { "Future" },
      return_type = dart_fn_return_type(&self.return_type),
      fn_name = self.fn_name,
      fn_params = if self.dart_outer_params.is_empty() {
        String::new()
      } else {
        format!("{{{}}}", self.dart_outer_params)
      },
      asink = if self.is_stream { " async*" } else { " async" }
    )
    .as_str();
    self
  }
  pub fn c_signature(&mut self) -> &mut Self {
    self.output += format!(
      "TaskHandle *{extern_c_fn_name}(int64_t port{extern_c_fn_types});",
      extern_c_fn_name = self.extern_c_fn_name,
      extern_c_fn_types = if self.extern_c_fn_types.is_empty() {
        String::new()
      } else {
        String::from(", ") + &self.extern_c_fn_types
      }
    )
    .as_str();
    self
  }

  pub fn body(&mut self, namespace: &str) -> &mut Self {
    self.output += format!(
      r#" {{
    final List<Pointer> _toFree = [];{fn_transforms}
    final _port = ReceivePort()..timeout(const Duration(milliseconds: 1000));

    Pointer<Int32>? _taskHandle;
    try {{
      _taskHandle = _bindings.{extern_c_fn_name}(_port.sendPort.nativePort{dart_inner_args});
      if (_taskHandle == null) {{
        throw {class_name}ApiError('Call to C failed');
      }}
    }} finally {{
      _toFree.forEach((ptr) => calloc.free(ptr));
    }}
"#,
      fn_transforms = if self.dart_transforms.is_empty() {
        String::new()
      } else {
        "\n    ".to_string() + &self.dart_transforms + ";"
      },
      extern_c_fn_name = self.extern_c_fn_name,
      dart_inner_args = if self.dart_inner_args.is_empty() {
        String::new()
      } else {
        String::from(", ") + &self.dart_inner_args
      },
      class_name = namespace.to_camel_case()
    )
    .as_str();
    self
  }

  pub fn body_return(&mut self, namespace: &str) -> &mut Self {
    self.output += if self.is_stream {
      format!(
        r#"
    try {{
      yield* _port.map((input) {{
        final deserializer = BincodeDeserializer(input as Uint8List);
        if (deserializer.deserializeBool()) {{
          return {return_de};
        }}
        throw {class_name}ApiError({error_de});
      }});
    }} finally {{
      if (_bindings.membrane_cancel_membrane_task(_taskHandle) < 1) {{
        throw {class_name}ApiError('Cancelation call to C failed');
      }}
    }}"#,
        return_de = self.deserializer(&self.return_type),
        error_de = self.deserializer(&self.error_type),
        class_name = namespace.to_camel_case()
      )
    } else {
      format!(
        r#"
    try {{
      final deserializer = BincodeDeserializer(await _port.first as Uint8List);
      if (deserializer.deserializeBool()) {{
        return {return_de};
      }}
      throw {class_name}ApiError({error_de});
    }} finally {{
      if (_bindings.membrane_cancel_membrane_task(_taskHandle) < 1) {{
        throw {class_name}ApiError('Cancelation call to C failed');
      }}
    }}"#,
        return_de = self.deserializer(&self.return_type),
        error_de = self.deserializer(&self.error_type),
        class_name = namespace.to_camel_case()
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

  fn deserializer(&self, ty: &str) -> String {
    let de;
    match ty {
      "String" => "deserializer.deserializeString()",
      "i32" => "deserializer.deserializeInt32()",
      "i64" => "deserializer.deserializeInt64()",
      "f32" => "deserializer.deserializeFloat32()",
      "f64" => "deserializer.deserializeFloat64()",
      "bool" => "deserializer.deserializeBool()",
      "()" => "null",
      ty if ty == "Option" => {
        panic!(
          "Option is not supported as a bare return type. Return the inner type from {} instead",
          self.fn_name
        )
      }
      _ => {
        de = format!("{}.deserialize(deserializer)", ty);
        &de
      }
    }
    .to_string()
  }
}

#[doc(hidden)]
pub struct TaskHandle(pub ::futures::future::AbortHandle);

#[doc(hidden)]
#[no_mangle]
pub unsafe extern "C" fn membrane_cancel_membrane_task(task_handle: *mut TaskHandle) -> i32 {
  // turn the pointer back into a box and Rust will drop it when it goes out of scope
  let handle = Box::from_raw(task_handle);
  handle.0.abort();

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
