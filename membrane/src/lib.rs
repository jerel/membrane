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
pub use membrane_macro::async_dart;
#[doc(hidden)]
pub use serde_reflection;

use heck::CamelCase;
use serde_reflection::{Samples, Tracer, TracerConfig};
use std::{collections::HashMap, io::Write};

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

inventory::collect!(DeferredTrace);

pub struct Membrane {
  package_name: String,
  destination: String,
  library: String,
  namespaces: Vec<String>,
  namespaced_registry: HashMap<String, Tracer>,
  namespaced_fn_registry: HashMap<String, Vec<Function>>,
  generated: bool,
}

impl Membrane {
  #[allow(clippy::new_without_default)]
  pub fn new() -> Self {
    let mut namespaces = vec![];
    let mut namespaced_registry = HashMap::new();
    let mut namespaced_samples = HashMap::new();
    let mut namespaced_fn_registry = HashMap::new();
    for item in inventory::iter::<DeferredTrace> {
      namespaces.push(item.namespace.clone());

      let mut entry = namespaced_registry
        .entry(item.namespace.clone())
        .or_insert_with(|| Tracer::new(TracerConfig::default()));

      let mut samples = namespaced_samples
        .entry(item.namespace.clone())
        .or_insert_with(|| Samples::new());

      (item.trace)(&mut entry, &mut samples);

      namespaced_fn_registry
        .entry(item.namespace.clone())
        .or_insert_with(Vec::new)
        .push(item.function.clone());
    }

    namespaces.sort();
    namespaces.dedup();

    // this might be useful for printing warnings
    // incomplete_enums: BTreeSet::new()
    Self {
      package_name: "".to_string(),
      destination: "./membrane_output".to_string(),
      library: "libmembrane".to_string(),
      namespaced_registry,
      namespaced_fn_registry,
      namespaces,
      generated: false,
    }
  }

  ///
  /// The directory for the pub package output. The basename will be the name of the pub package.
  pub fn package_destination_dir(&mut self, path: &str) -> &mut Self {
    // allowing an empty path could result in data loss in a directory named `lib`
    if path.is_empty() {
      panic!("package_destination_dir() cannot be called with an empty path");
    }
    self.destination = path.trim_end_matches('/').to_string();

    self
  }

  ///
  /// The name of the generate package.
  pub fn package_name(&mut self, name: &str) -> &mut Self {
    self.package_name = name.to_string();
    self
  }

  ///
  /// The name of the dylib or so that the Rust project produces. Membrane
  /// generated code will load this library at runtime.
  pub fn using_lib(&mut self, name: &str) -> &mut Self {
    self.library = name.to_string();
    self
  }

  ///
  /// Write the pub package to the destination set with `package_destination_dir`.
  /// Existing Dart files in this directory may be deleted during this operation.
  pub fn create_pub_package(&mut self) -> &mut Self {
    use serde_generate::SourceInstaller;

    #[cfg(all(
      any(not(debug_assertions), feature = "skip-generate"),
      not(feature = "generate")
    ))]
    return self;

    // remove all previously generated type and header files
    let _ = std::fs::remove_dir_all(self.destination.clone() + "/lib");
    let _ = std::fs::remove_file(self.destination.clone() + "/pubspec.yaml");
    std::fs::create_dir_all(self.destination.clone() + "/lib/src").unwrap();

    let dest_path = std::path::PathBuf::from(self.destination.clone());
    let installer = serde_generate::dart::Installer::new(dest_path.clone());
    installer.install_serde_runtime().unwrap();
    installer.install_bincode_runtime().unwrap();

    for namespace in self.namespaces.iter() {
      let config = serde_generate::CodeGeneratorConfig::new(namespace.to_string())
        .with_encodings(vec![serde_generate::Encoding::Bincode]);

      let tracer = self.namespaced_registry.remove(namespace).unwrap();
      let registry = tracer.registry().unwrap();
      let generator = serde_generate::dart::CodeGenerator::new(&config);
      generator.output(dest_path.clone(), &registry).unwrap();
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
      self.destination.rsplit('/').next().unwrap()
    } else {
      self.package_name.as_str()
    };
    let path = self.destination.clone() + "/pubspec.yaml";
    let re = regex::Regex::new(r"^name:(.*?)\n").unwrap();
    match std::fs::read_to_string(&path) {
      Ok(old) => {
        let pubspec = re
          .replace(
            old.as_str().trim(),
            "name: ".to_string() + package_name + "\n",
          )
          .to_string();

        let extra_deps = r#"
  ffi: ^1.1.2

dev_dependencies:
  ffigen: ^3.0.0
"#;
        std::fs::write(path, pubspec + extra_deps).expect("pubspec could not be written");
      }
      Err(_) => (),
    }

    self
  }

  fn write_ffigen_config(&mut self) -> &mut Self {
    let config = r#"
name: 'NativeLibrary'
description: 'Auto generated bindings for Dart types'
output: './lib/src/ffi_bindings.dart'
headers:
  entry-points:
    - 'lib/src/*/*.h'
"#;

    let path = self.destination.clone() + "/ffigen.yaml";
    std::fs::write(&path, config).unwrap_or_else(|_| {
      panic!("unable to write ffigen config {}", path);
    });

    self
  }

  fn write_header(&mut self, namespace: String) -> &mut Self {
    use std::io::prelude::*;
    let path = self.namespace_path(namespace.clone()) + "/" + &namespace + ".h";
    let fns = self.namespaced_fn_registry.get(&namespace).unwrap();

    let head = r#"
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

"#;

    let mut buffer =
      std::fs::File::create(path.clone()).expect("header could not be written at namespace path");
    buffer.write_all(head.as_bytes()).unwrap_or_else(|_| {
      panic!("unable to write C header file {}", path);
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
      .arg("format")
      .arg(".")
      .output();

    self
  }

  fn create_loader(&mut self) -> &mut Self {
    let base_class = format!(
      r#"import 'dart:ffi';
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

    let path = self.destination.clone() + "/lib/src/loader.dart";
    std::fs::write(path, base_class).unwrap();

    self
  }

  fn create_class(&mut self, namespace: String) -> &mut Self {
    use std::io::prelude::*;
    let path = self.destination.clone() + "/lib/" + &namespace + ".dart";
    let fns = self.namespaced_fn_registry.get(&namespace).unwrap();

    let head = format!(
      r#"import 'dart:ffi';
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

  fn namespace_path(&mut self, namespace: String) -> String {
    self.destination.clone() + "/lib/src/" + &namespace
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
      return_type = self.return_type,
      fn_name = self.fn_name,
      fn_params = if self.dart_outer_params.is_empty() {
        String::new()
      } else {
        format!("{{{}}}", self.dart_outer_params)
      },
      asink = if self.is_stream { "" } else { " async" }
    )
    .as_str();
    self
  }
  pub fn c_signature(&mut self) -> &mut Self {
    self.output += format!(
      "int32_t {extern_c_fn_name}(int64_t port{extern_c_fn_types});",
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
      r#" {{{fn_transforms}
    final port = ReceivePort()..timeout(const Duration(milliseconds: 1000));

    if (_bindings.{extern_c_fn_name}(port.sendPort.nativePort{dart_inner_args}) < 1) {{
      throw {class_name}ApiError('Call to C failed');
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
    return port.map((input) {{
      final deserializer = BincodeDeserializer(input as Uint8List);
      if (deserializer.deserializeBool()) {{
        return {return_type}.deserialize(deserializer);
      }}
      throw {class_name}ApiError({error_type}.deserialize(deserializer));
    }});"#,
        return_type = self.return_type,
        error_type = self.error_type,
        class_name = namespace.to_camel_case()
      )
    } else {
      format!(
        r#"
    final deserializer = BincodeDeserializer(await port.first as Uint8List);
    if (deserializer.deserializeBool()) {{
      return {return_type}.deserialize(deserializer);
    }}
    throw {class_name}ApiError({error_type}.deserialize(deserializer));"#,
        return_type = self.return_type,
        error_type = self.error_type,
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
      .write_all(&self.output.as_bytes())
      .expect("function could not be written at path");
    self
  }
}

#[doc(hidden)]
#[macro_export]
macro_rules! error {
  ($result:expr) => {
    error!($result, 0);
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
    cstr!($ptr, 0)
  };
  ($ptr:expr, $error:expr) => {{
    ::membrane::ffi_helpers::null_pointer_check!($ptr);
    error!(unsafe { CStr::from_ptr($ptr).to_str() }, $error)
  }};
}
