// Native structural diagnostics share the file bridge's exact constructors.
// Only the final validity gate is deferred, so invalid copies can be inspected.
// All shape/layout/ownership checks remain active; file writing uses the default
// validating constructors and never takes this diagnostic path.
namespace {
class CheckTextLog final : public ON_TextLog {
 public:
  std::string text;
  bool truncated = false;
 private:
  static constexpr size_t limit = 8192;
  void AppendText(const char* value) override {
    if (value == nullptr) return;
    const size_t length = std::strlen(value);
    const size_t remaining = limit - text.size();
    text.append(value, std::min(length, remaining));
    truncated = truncated || length > remaining;
  }
  void AppendText(const wchar_t* value) override {
    if (value != nullptr) AppendText(ON_String(value).Array());
  }
};
}

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
    std::unique_ptr<ON_Object> geometry;
    if (source->object_type == VIBO_OBJECT_INSTANCE) {
      if (definition_uuid == nullptr || source->coordinate_count != 16 ||
          !finite_coordinates(source->coordinates, source->coordinate_count)) {
        set_error(diagnostics, capacity, "Instance checks require a definition UUID and affine placement");
        return 0;
      }
      auto instance = std::make_unique<ON_InstanceRef>();
      instance->m_instance_definition_uuid = ON_UuidFromString(definition_uuid);
      for (int r=0; r<4; ++r) for (int c=0; c<4; ++c)
        instance->m_xform[r][c] = source->coordinates[4*r+c];
      geometry = std::move(instance);
    } else {
      geometry.reset(geometry_for(*source, error, false));
    }
    if (!geometry) {
      set_error(diagnostics, capacity, error.empty() ? "Native geometry transfer failed" : error);
      return 0;
    }
    CheckTextLog log;
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
