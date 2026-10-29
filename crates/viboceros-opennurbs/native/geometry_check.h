// Native structural diagnostics share the file bridge's exact constructors.
// Only the final validity gate is deferred, so invalid copies can be inspected.
// All shape/layout/ownership checks remain active; file writing uses the default
// validating constructors and never takes this diagnostic path.
extern "C" int32_t vibo_geometry_check(const ViboWriteObject* source,
    const char* definition_uuid, int32_t* valid, char* diagnostics, size_t capacity) {
  if (valid != nullptr) *valid = -1;
  set_error(diagnostics, capacity, "");
  try {
    if (source == nullptr || valid == nullptr) {
      set_error(diagnostics, capacity, "Missing geometry check arguments");
      return 0;
    }
    vibo_begin_open_nurbs();
    std::string error;
    auto geometry = diagnostic_geometry(*source, definition_uuid, error);
    if (!geometry) {
      set_error(diagnostics, capacity, error.empty() ? "Native geometry transfer failed" : error);
      return 0;
    }
    GeometryTextLog log;
    *valid = geometry->IsValid(&log) ? 1 : 0;
    if (*valid == 0 && log.text.empty()) log.text = "Invalid OpenNURBS geometry";
    if (log.truncated) log.text += "\n[diagnostics truncated]";
    set_error(diagnostics, capacity, log.text);
    return 1;
  } catch (const std::exception& error) {
    set_error(diagnostics, capacity, std::string("Native check failed: ") + error.what());
  } catch (...) {
    set_error(diagnostics, capacity, "Unknown native geometry check failure");
  }
  if (valid != nullptr) *valid = -1;
  return 0;
}
