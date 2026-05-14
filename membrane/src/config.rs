use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct DartConfig {
  pub(crate) versions: HashMap<&'static str, &'static str>,
  pub(crate) logger: DartLoggerConfig,
  pub(crate) v1_import_style: Vec<&'static str>,
}

impl Default for DartConfig {
  fn default() -> Self {
    Self {
      versions: HashMap::from([
        ("sdk", ">=3.0.0 <4.0.0"),
        ("ffi", "^2.1.0"),
        ("ffigen", "^9.0.0"),
        ("logger", "^1.1.0"),
      ]),
      logger: DartLoggerConfig::default(),
      v1_import_style: vec![],
    }
  }
}

impl DartConfig {
  /// Override the default version strings that are set in the generated pub package.
  ///
  /// Valid options: sdk, ffi, ffigen, logger.
  pub fn set_version(&mut self, name: &'static str, version: &'static str) {
    self
      .versions
      .insert(name, version)
      .expect("An unknown version cannot be set. Valid options: sdk, ffi, ffigen, logger.");
  }

  /// This config allows the logger code that is injected into generated code to be customized. Using this
  /// you can change the logging dependency, adjust the version, and change the names of logger methods.
  pub fn logger(&mut self, dart_config: DartLoggerConfig) {
    self.logger = dart_config;
  }

  /// This config exists temporarily as a tool to incrementally migrate large codebases away from the old automatic
  /// re-export behavior one namespace at a time. It will be removed in a future version. Add namespaces to
  /// this config to retain the old Dart import/export behavior for borrowed types.
  ///
  /// In the old behavior namespace `a` borrowing a type with `borrow = "b::Foo"` would add `export './b/b.dart show Foo;` to
  /// the implementation file. In some situations this could result in conflicting type names in app files trying to use a
  /// Membrane-generated API.
  ///
  /// In the new behavior a namespace only exports its own types publicly and the developer must import borrowed
  /// types (if needed) in app code. This means that types which use types from other namespaces will work but the app
  /// scope won't be polluted with needlessly exported type names.
  pub fn v1_import_style(&mut self, namespaces: Vec<&'static str>) {
    self.v1_import_style = namespaces;
  }
}

#[derive(Debug, Clone)]
pub struct DartLoggerConfig {
  pub dependency_name: &'static str,
  pub import_path: &'static str,
  pub instance: &'static str,
  pub info_log_fn: &'static str,
  pub fine_log_fn: &'static str,
}

impl Default for DartLoggerConfig {
  fn default() -> Self {
    Self {
      dependency_name: "logging",
      import_path: "package:logging/logging.dart",
      instance: "Logger('membrane')",
      info_log_fn: "info",
      fine_log_fn: "fine",
    }
  }
}
