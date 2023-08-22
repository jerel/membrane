pub fn create_ffi_loader(library: &str, dart_config: &crate::DartConfig) -> String {
  format!(
    r#"// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
import 'dart:ffi';
import 'dart:io' show Platform;
import 'package:ffi/ffi.dart';
import '{logger_path}';

import './ffi_bindings.dart' as ffi_bindings;

DynamicLibrary _open() {{
  if (Platform.isLinux) {{
    {logger}.{info_logger}('Opening native library {lib}.so');
    return DynamicLibrary.open('{lib}.so');
  }}
  if (Platform.isAndroid) {{
    {logger}.{info_logger}('Opening native library {lib}.so');
    return DynamicLibrary.open('{lib}.so');
  }}
  if (Platform.isIOS) {{
    {logger}.{info_logger}('Creating dynamic library {lib}');
    return DynamicLibrary.executable();
  }}
  if (Platform.isMacOS) {{
    {logger}.{info_logger}('Opening native library {lib}.dylib');
    return DynamicLibrary.open('{lib}.dylib');
  }}
  if (Platform.isWindows) {{
    {logger}.{info_logger}('Opening native library {lib}.dll');
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
  {logger}.{info_logger}('Initializing FFI bindings');
  final bindings = ffi_bindings.NativeLibrary(dl);
  final storeDartPostCobject =
      dl.lookupFunction<_StoreDartPostCobjectC, _StoreDartPostCobjectDart>(
    'store_dart_post_cobject',
  );

  {logger}.{debug_logger}('Initializing Dart_PostCObject');
  storeDartPostCobject(NativeApi.postCObject);

  final ptr = bindings.membrane_metadata_version();
  final version = ptr.cast<Utf8>().toDartString();
  bindings.membrane_free_membrane_string(ptr);
  final msg = "Successfully loaded '{lib}' which was built at version '$version'.";
  {logger}.{info_logger}(msg);

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
    lib = library,
    logger_path = dart_config.logger_import_path,
    logger = dart_config.logger,
    info_logger = dart_config.info_log_fn,
    debug_logger = dart_config.debug_log_fn,
  )
}

pub fn create_web_loader(_library: &str) -> String {
  "// AUTO GENERATED FILE, DO NOT EDIT
//
// Generated by `membrane`
_connect() {}

bool bindingsLoaded = false;
final bindings = _connect();"
    .to_string()
}
