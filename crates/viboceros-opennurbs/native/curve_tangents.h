// Batch public tangent evaluation using the shared exact rational constructor.
extern "C" int32_t vibo_curve_tangents(uint32_t degree, size_t count,
    const double* controls, size_t knot_count, const double* knots,
    size_t parameter_count, const double* parameters, int32_t* found,
    double* tangents, char* error, size_t capacity) {
  set_error(error, capacity, "");
  try {
    if (!controls || !knots || count < 2 || count > 1000000 || degree == 0 ||
        degree > 64 || degree >= count || knot_count != count + degree + 1 ||
        parameter_count > 1000000 ||
        (parameter_count && (!parameters || !found || !tangents))) {
      set_error(error, capacity, "Invalid curve tangent buffers"); return 0;
    }
    vibo_begin_open_nurbs();
    ViboWriteObject source{};
    source.object_type = VIBO_OBJECT_NURBS_CURVE;
    source.degree_u = degree; source.control_point_count_u = count;
    source.rational_curve = 1;
    source.coordinates = controls; source.coordinate_count = count * 4;
    source.knots_u = knots; source.knot_u_count = knot_count;
    std::string message;
    std::unique_ptr<ON_Object> owned(geometry_for(source, message));
    const auto* curve = ON_Curve::Cast(owned.get());
    if (!curve) {
      set_error(error, capacity, message.empty() ? "Invalid curve tangent source" : message); return 0;
    }
    const auto domain = curve->Domain();
    for (size_t i=0; i<parameter_count; ++i) {
      found[i] = 0;
      if (!ON_IsValid(parameters[i]) || !domain.Includes(parameters[i])) {
        set_error(error, capacity, "Curve tangent parameter is outside the domain"); return 0;
      }
      ON_3dPoint point; ON_3dVector tangent;
      if (!curve->EvTangent(parameters[i], point, tangent)) continue;
      if (!tangent.IsValid()) {
        set_error(error, capacity, "Invalid native curve tangent"); return 0;
      }
      found[i] = 1;
      for (int axis=0; axis<3; ++axis) tangents[3*i+axis] = tangent[axis];
    }
    return 1;
  } catch (const std::exception& exception) {
    set_error(error, capacity, std::string("Curve tangent query failed: ") + exception.what());
  } catch (...) {set_error(error, capacity, "Unknown curve tangent query failure");}
  return 0;
}
