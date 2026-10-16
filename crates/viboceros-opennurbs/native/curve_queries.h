// Reuse the file bridge's rational constructor and OpenNURBS' plane query.
extern "C" int32_t vibo_curve_plane_normal(uint32_t degree, size_t count,
    const double* controls, size_t knot_count, const double* knots,
    double tolerance, int32_t* planar, double* normal, char* error, size_t capacity) {
  if (planar) *planar = -1;
  set_error(error, capacity, "");
  try {
    if (!planar || !normal || !std::isfinite(tolerance) || tolerance < 0.0 ||
        count > static_cast<size_t>(std::numeric_limits<int>::max()) ||
        count > std::numeric_limits<size_t>::max() / 4) {
      set_error(error, capacity, "Invalid curve plane query arguments");
      return 0;
    }
    vibo_begin_open_nurbs();
    ViboWriteObject source{};
    source.object_type = VIBO_OBJECT_NURBS_CURVE;
    source.degree_u = degree;
    source.control_point_count_u = count;
    source.coordinates = controls;
    source.coordinate_count = count * 4;
    source.knots_u = knots;
    source.knot_u_count = knot_count;
    std::string message;
    std::unique_ptr<ON_Object> object(geometry_for(source, message));
    const ON_Curve* curve = ON_Curve::Cast(object.get());
    if (!curve) {
      set_error(error, capacity, message.empty() ? "Curve transfer failed" : message);
      return 0;
    }
    ON_Plane plane;
    const bool found = curve->IsPlanar(&plane, tolerance);
    if (found && !plane.IsValid()) {
      set_error(error, capacity, "OpenNURBS returned an invalid curve plane");
      return 0;
    }
    if (found) for (int i=0; i<3; ++i) normal[i] = plane.zaxis[i];
    *planar = found ? 1 : 0;
    return 1;
  } catch (const std::exception& exception) {
    set_error(error, capacity, std::string("Curve plane query failed: ") + exception.what());
  } catch (...) {
    set_error(error, capacity, "Unknown curve plane query failure");
  }
  return 0;
}
