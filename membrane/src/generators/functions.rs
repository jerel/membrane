use crate::{ContainerFormat, Function, Membrane, Registry, VariantFormat};
use membrane_types::{dart::dart_type, heck::CamelCase};
use std::io::Write;

///
/// The types of interfaces that we generate. FFI and Web are used on
/// the platforms of the same name and C is used to generate headers for use by FFI.
///

pub(crate) struct Ffi {
  output: String,
  fun: Function,
}

pub(crate) struct Web {
  output: String,
  fun: Function,
}

pub(crate) struct C {
  output: String,
  fun: Function,
}

///
///
/// Convert a Function struct into a string representation of a Dart function
///
///

pub(crate) trait Builder {
  fn new(input: &Function) -> Self;
  fn build(&mut self, config: &Membrane) -> Self;
  fn as_bytes(&self) -> &[u8];
}

impl Builder for Ffi {
  fn new(input: &Function) -> Self {
    Self {
      output: "".to_string(),
      fun: input.clone(),
    }
  }

  fn as_bytes(&self) -> &[u8] {
    self.output.as_bytes()
  }

  fn build(&mut self, config: &Membrane) -> Ffi {
    let enum_registry = config
      .namespaced_enum_registry
      .get(&self.fun.namespace)
      .unwrap()
      // we've already inspected the registry for incomplete enums, now we'll have only valid ones
      .as_ref().unwrap();

    Ffi {
      output: self
        .begin()
        .signature()
        .body()
        .body_return(enum_registry, config)
        .end()
        .output
        .clone(),
      fun: self.fun.clone(),
    }
  }
}

impl Builder for Web {
  fn new(input: &Function) -> Self {
    Self {
      output: "".to_string(),
      fun: input.clone(),
    }
  }

  fn as_bytes(&self) -> &[u8] {
    self.output.as_bytes()
  }

  fn build(&mut self, _config: &Membrane) -> Web {
    Web {
      output: self.begin().signature().body().end().output.clone(),
      fun: self.fun.clone(),
    }
  }
}

impl Builder for C {
  fn new(input: &Function) -> Self {
    Self {
      output: "".to_string(),
      fun: input.clone(),
    }
  }

  fn as_bytes(&self) -> &[u8] {
    self.output.as_bytes()
  }

  fn build(&mut self, _config: &Membrane) -> C {
    C {
      output: self.begin().signature().output.clone(),
      fun: self.fun.clone(),
    }
  }
}

///
///
/// Write a string representation to the given buffer
///
///

pub(crate) trait Writable: Builder {
  fn write(&self, mut buffer: &std::fs::File) {
    buffer
      .write_all(self.as_bytes())
      .expect("function could not be written at path");
  }
}

impl Writable for Ffi {}
impl Writable for Web {}
impl Writable for C {}

impl Function {
  fn begin(&mut self) -> String {
    "\n".to_string()
  }

  fn signature(&mut self) -> String {
    format!(
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
  }

  fn end(&mut self) -> String {
    "\n  }\n".to_string()
  }

  #[allow(clippy::only_used_in_recursion)]
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
      ["Option", ..] => {
        de = format!(
          "() {{
            if (deserializer.deserializeOptionTag()) {{
              return {};
            }}
            return null;
          }}();",
          self.deserializer(&ty[1..], enum_tracer_registry, config)
        );
        &de
      }
      [ty, ..] => {
        de = match enum_tracer_registry.get(ty) {
          Some(ContainerFormat::Enum(variants))
            if config.c_style_enums
              && variants.values().all(|f| f.value == VariantFormat::Unit) =>
          {
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

trait Callable {
  fn begin(&mut self) -> &mut Self;
  fn signature(&mut self) -> &mut Self;
  fn body(&mut self) -> &mut Self;
  fn body_return(&mut self, enum_tracer_registry: &Registry, config: &Membrane) -> &mut Self;
  fn end(&mut self) -> &mut Self;
}

impl Callable for Ffi {
  fn begin(&mut self) -> &mut Self {
    self.output += &self.fun.begin();
    self
  }

  fn signature(&mut self) -> &mut Self {
    self.output += &self.fun.signature();
    self
  }

  fn body(&mut self) -> &mut Self {
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
      disable_logging = if self.fun.disable_logging {
        "final _loggingDisabled = true;"
      } else {
        ""
      },
      fn_transforms = if self.fun.dart_transforms.is_empty() {
        String::new()
      } else {
        "\n    ".to_string() + &self.fun.dart_transforms + ";"
      },
      receive_port = if self.fun.is_sync {
        ""
      } else {
        "\n    final _port = ReceivePort();"
      },
      extern_c_fn_name = self.fun.extern_c_fn_name,
      fn_name = self.fun.fn_name,
      native_port = if self.fun.is_sync {
        ""
      } else {
        "_port.sendPort.nativePort"
      },
      dart_inner_args = if self.fun.dart_inner_args.is_empty() {
        String::new()
      } else if self.fun.is_sync {
        String::new() + &self.fun.dart_inner_args
      } else {
        String::from(", ") + &self.fun.dart_inner_args
      },
      class_name = self.fun.namespace.to_camel_case()
    )
    .as_str();
    self
  }

  fn end(&mut self) -> &mut Self {
    self.output += &self.fun.end();
    self
  }

  fn body_return(&mut self, enum_tracer_registry: &Registry, config: &Membrane) -> &mut Self {
    self.output += if self.fun.is_sync {
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
        return_de = self.fun.deserializer(&self.fun.return_type, enum_tracer_registry, config),
        error_de = self.fun.deserializer(&self.fun.error_type, enum_tracer_registry, config),
        class_name = self.fun.namespace.to_camel_case(),
        fn_name = self.fun.fn_name,
      )
    } else if self.fun.is_stream {
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
        return_de = self.fun.deserializer(&self.fun.return_type, enum_tracer_registry, config),
        error_de = self.fun.deserializer(&self.fun.error_type, enum_tracer_registry, config),
        class_name = self.fun.namespace.to_camel_case(),
        fn_name = self.fun.fn_name,
        timeout = if let Some(val) = self.fun.timeout {
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
        return_de = self.fun.deserializer(&self.fun.return_type, enum_tracer_registry, config),
        error_de = self.fun.deserializer(&self.fun.error_type, enum_tracer_registry, config),
        class_name = self.fun.namespace.to_camel_case(),
        fn_name = self.fun.fn_name,
        timeout = if let Some(val) = self.fun.timeout {
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
}

impl Callable for Web {
  fn begin(&mut self) -> &mut Self {
    self.output += &self.fun.begin();
    self
  }

  fn signature(&mut self) -> &mut Self {
    self.output += &self.fun.signature();
    self
  }

  fn body(&mut self) -> &mut Self {
    self.output += "{
      throw UnimplementedError();";

    self
  }

  fn body_return(&mut self, _enum_tracer_registry: &Registry, _config: &Membrane) -> &mut Self {
    self
  }

  fn end(&mut self) -> &mut Self {
    self.output += &self.fun.end();
    self
  }
}

impl Callable for C {
  fn begin(&mut self) -> &mut Self {
    self.output += &self.fun.begin();
    self
  }

  fn signature(&mut self) -> &mut Self {
    self.output += format!(
      "MembraneResponse {extern_c_fn_name}({port}{extern_c_fn_types});",
      extern_c_fn_name = self.fun.extern_c_fn_name,
      port = if self.fun.is_sync { "" } else { "int64_t port" },
      extern_c_fn_types = if self.fun.extern_c_fn_types.is_empty() {
        String::new()
      } else if self.fun.is_sync {
        String::new() + &self.fun.extern_c_fn_types
      } else {
        String::from(", ") + &self.fun.extern_c_fn_types
      }
    )
    .as_str();
    self
  }

  fn body(&mut self) -> &mut Self {
    self
  }

  fn body_return(&mut self, _enum_tracer_registry: &Registry, _config: &Membrane) -> &mut Self {
    self
  }

  fn end(&mut self) -> &mut Self {
    self
  }
}
