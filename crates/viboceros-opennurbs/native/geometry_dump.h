// OpenNURBS owns the technical representation of each geometry type.
extern "C" int32_t vibo_geometry_dump(const ViboWriteObject* source,
    const char* definition_uuid, int32_t* truncated, char* report, size_t capacity) {
  if (truncated != nullptr) *truncated = 0;
  set_error(report, capacity, "");
  try {
    if (source == nullptr || truncated == nullptr || report == nullptr ||
        capacity < 2 || capacity > 1024 * 1024) {
      set_error(report, capacity, "Invalid geometry dump arguments or output capacity");
      return 0;
    }
    vibo_begin_open_nurbs();
    std::string error;
    auto geometry = diagnostic_geometry(*source, definition_uuid, error);
    if (!geometry) {
      set_error(report, capacity, error.empty() ? "Native geometry transfer failed" : error);
      return 0;
    }
    GeometryTextLog log(capacity - 1);
    log.SetLevelOfDetail(ON_TextLog::LevelOfDetail::Maximum);
    log.SetDoubleFormat("%.17g");
    geometry->Dump(log);
    // ON_InstanceRef inherits ON_Object's generic Dump. Include its actual
    // reference fields explicitly; no definition expansion or geometry edit.
    if (const auto* instance = ON_InstanceRef::Cast(geometry.get())) {
      log.Print("Instance definition: ");
      log.Print(instance->m_instance_definition_uuid);
      log.Print("\nTransform:\n");
      log.Print(instance->m_xform);
    }
    *truncated = log.truncated ? 1 : 0;
    set_error(report, capacity, log.text);
    return 1;
  } catch (const std::exception& error) {
    set_error(report, capacity, std::string("Native geometry dump failed: ") + error.what());
  } catch (...) {
    set_error(report, capacity, "Unknown native geometry dump failure");
  }
  return 0;
}
