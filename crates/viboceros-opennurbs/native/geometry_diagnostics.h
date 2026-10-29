// Shared bounded logging and owned native copies for read-only diagnostics.
namespace {
class GeometryTextLog final : public ON_TextLog {
 public:
  explicit GeometryTextLog(size_t limit = 8192) : limit_(limit) {}
  std::string text;
  bool truncated = false;
 private:
  size_t limit_;
  void AppendText(const char* value) override {
    if (value == nullptr) return;
    const size_t length = std::strlen(value);
    const size_t remaining = limit_ - text.size();
    text.append(value, std::min(length, remaining));
    truncated = truncated || length > remaining;
  }
  void AppendText(const wchar_t* value) override {
    if (value != nullptr) AppendText(ON_String(value).Array());
  }
};

std::unique_ptr<ON_Object> diagnostic_geometry(const ViboWriteObject& source,
    const char* definition_uuid, std::string& error) {
  if (source.object_type != VIBO_OBJECT_INSTANCE)
    return std::unique_ptr<ON_Object>(geometry_for(source, error, false));
  if (definition_uuid == nullptr || source.coordinate_count != 16 ||
      !finite_coordinates(source.coordinates, source.coordinate_count)) {
    error = "Instance diagnostics require a definition UUID and affine placement";
    return nullptr;
  }
  auto instance = std::make_unique<ON_InstanceRef>();
  instance->m_instance_definition_uuid = ON_UuidFromString(definition_uuid);
  for (int r=0; r<4; ++r) for (int c=0; c<4; ++c)
    instance->m_xform[r][c] = source.coordinates[4*r+c];
  return instance;
}
}
