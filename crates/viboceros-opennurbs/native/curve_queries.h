// Reuse exact public OpenNURBS recognition rather than fitting a polyline.
extern "C" int32_t vibo_curve_polyline_vertices(uint32_t degree, size_t count,
    const double* controls, size_t knot_count, const double* knots,
    size_t* retained, double* result, char* error, size_t capacity) {
  if (retained) *retained = 0;
  set_error(error, capacity, "");
  try {
    if (!retained || !result || count < 2 || count > 1000000) {
      set_error(error, capacity, "Invalid polyline recognition buffers");
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
      set_error(error, capacity, message.empty() ? "Polyline curve transfer failed" : message);
      return 0;
    }
    ON_SimpleArray<ON_3dPoint> points;
    const int found = curve->IsPolyline(&points, nullptr);
    if (found == 0) return 1;
    if (found < 2 || found != points.Count() || static_cast<size_t>(found) > count) {
      set_error(error, capacity, "Invalid native polyline recognition result");
      return 0;
    }
    for (int i=0; i<found; ++i) for (int j=0; j<3; ++j)
      result[3*i+j] = points[i][j];
    *retained = static_cast<size_t>(found);
    return 1;
  } catch (const std::exception& exception) {
    set_error(error, capacity, std::string("Polyline recognition failed: ") + exception.what());
  } catch (...) {set_error(error, capacity, "Unknown polyline recognition failure");}
  return 0;
}

// Reuse the file bridge's rational constructor and OpenNURBS' plane query.
extern "C" int32_t vibo_arc_through_points(const double* points, int32_t* found,
    double* result, char* error, size_t capacity) {
  if (found) *found = -1;
  set_error(error, capacity, "");
  try {
    if (!points || !found || !result) {
      set_error(error, capacity, "Invalid arc construction buffers");
      return 0;
    }
    for (int i=0; i<9; ++i) if (!std::isfinite(points[i])) {
      set_error(error, capacity, "Nonfinite arc construction point");
      return 0;
    }
    vibo_begin_open_nurbs();
    ON_Arc arc;
    if (!arc.Create(ON_3dPoint(points), ON_3dPoint(points+3), ON_3dPoint(points+6)) || !arc.IsValid()) {
      *found = 0;
      return 1;
    }
    for (int i=0; i<3; ++i) {
      result[i] = arc.plane.origin[i];
      result[3+i] = arc.plane.xaxis[i];
      result[6+i] = arc.plane.yaxis[i];
      result[9+i] = arc.plane.zaxis[i];
    }
    result[12] = arc.radius;
    result[13] = arc.AngleRadians();
    *found = 1;
    return 1;
  } catch (const std::exception& exception) {
    set_error(error, capacity, std::string("Arc construction failed: ") + exception.what());
  } catch (...) {
    set_error(error, capacity, "Unknown arc construction failure");
  }
  return 0;
}

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

// Explicit analytic approximation; general affine geometry transforms retain
// their exact NURBS image instead. ON_Circle::Transform is the public policy
// used by native parameter editing of analytic curves inside block references.
extern "C" int32_t vibo_transform_analytic_circle(const double* frame,
    const double* matrix, double* result, char* error, size_t capacity) {
  set_error(error, capacity, "");
  try {
    if (!frame || !matrix || !result) {
      set_error(error, capacity, "Invalid analytic circle buffers"); return 0;
    }
    for (int i=0; i<10; ++i) if (!std::isfinite(frame[i])) {
      set_error(error, capacity, "Nonfinite analytic circle frame"); return 0;
    }
    ON_Xform xform;
    for (int i=0; i<16; ++i) {
      if (!std::isfinite(matrix[i])) {
        set_error(error, capacity, "Nonfinite analytic circle transform"); return 0;
      }
      xform[i/4][i%4] = matrix[i];
    }
    if (matrix[12]!=0. || matrix[13]!=0. || matrix[14]!=0. || matrix[15]!=1.) {
      set_error(error, capacity, "Analytic circle transform must be affine"); return 0;
    }
    vibo_begin_open_nurbs();
    ON_Circle circle(ON_Plane(ON_3dPoint(frame), ON_3dVector(frame+3), ON_3dVector(frame+6)), frame[9]);
    if (!circle.IsValid() || !circle.Transform(xform) || !circle.IsValid()) {
      set_error(error, capacity, "Analytic circle transformation failed"); return 0;
    }
    for (int i=0; i<3; ++i) {
      result[i] = circle.plane.origin[i];
      result[3+i] = circle.plane.xaxis[i];
      result[6+i] = circle.plane.zaxis[i];
    }
    result[9] = circle.radius;
    return 1;
  } catch (const std::exception& exception) {
    set_error(error, capacity, std::string("Analytic circle transform failed: ") + exception.what());
  } catch (...) {
    set_error(error, capacity, "Unknown analytic circle transform failure");
  }
  return 0;
}

extern "C" int32_t vibo_curve_analytic_form(uint32_t degree, size_t count,
    const double* controls, size_t knot_count, const double* knots,
    double tolerance, int32_t* kind, double* result, char* error, size_t capacity) {
  if (kind) *kind = -1;
  set_error(error, capacity, "");
  try {
    if (!kind || !result || !std::isfinite(tolerance) || tolerance < 0.0 ||
        count > static_cast<size_t>(std::numeric_limits<int>::max()) ||
        count > std::numeric_limits<size_t>::max() / 4) {
      set_error(error, capacity, "Invalid analytic curve query arguments"); return 0;
    }
    vibo_begin_open_nurbs();
    ViboWriteObject source{};
    source.object_type = VIBO_OBJECT_NURBS_CURVE;
    source.degree_u = degree; source.control_point_count_u = count;
    source.coordinates = controls; source.coordinate_count = count * 4;
    source.knots_u = knots; source.knot_u_count = knot_count;
    std::string message;
    std::unique_ptr<ON_Object> object(geometry_for(source, message));
    const ON_Curve* curve = ON_Curve::Cast(object.get());
    if (!curve) {set_error(error, capacity, message);return 0;}
    if (curve->IsLinear(tolerance)) {
      const ON_3dPoint a = curve->PointAtStart(), b = curve->PointAtEnd();
      for (int i=0; i<3; ++i) {result[i] = a[i];result[3+i] = b[i];}
      *kind = 1;return 1;
    }
    ON_Arc arc;
    if (curve->IsArc(nullptr,&arc,tolerance) && arc.IsValid()) {
      for (int i=0; i<3; ++i) {
        result[i] = arc.Center()[i];result[3+i] = arc.plane.xaxis[i];result[6+i] = arc.plane.zaxis[i];
      }
      result[9] = arc.Radius();result[10] = arc.Domain()[0];result[11] = arc.Domain()[1];
      *kind = 2;return 1;
    }
    *kind = 0;return 1;
  } catch (const std::exception& exception) {
    set_error(error, capacity, std::string("Analytic curve query failed: ") + exception.what());
  } catch (...) {set_error(error, capacity, "Unknown analytic curve query failure");}
  return 0;
}

// One borrowed three-point curve is reused for every decision, avoiding a
// NURBS allocation and a language boundary crossing for every polyline vertex.
extern "C" int32_t vibo_polyline_simplify_indices(size_t count, const double* points,
    double tolerance, size_t* indices, size_t* retained, char* error, size_t capacity) {
  if (retained) *retained = 0;
  set_error(error, capacity, "");
  try {
    if (!points || !indices || !retained || count < 2 || count > 1000000 ||
        !std::isfinite(tolerance) || tolerance < 0.) {
      set_error(error, capacity, "Invalid polyline simplification arguments");return 0;
    }
    for (size_t i=0; i<count*3; ++i) if (!std::isfinite(points[i])) {
      set_error(error, capacity, "Nonfinite polyline coordinate");return 0;
    }
    vibo_begin_open_nurbs();
    ON_3dPointArray scratch;
    for (int i=0; i<3; ++i) scratch.Append(ON_3dPoint::Origin);
    ON_PolylineCurve triple(scratch);
    size_t n = 0;
    for (size_t i=0; i<count; ++i) {
      while (n>=2) {
        triple.m_pline[0] = ON_3dPoint(points+3*indices[n-2]);
        triple.m_pline[1] = ON_3dPoint(points+3*indices[n-1]);
        triple.m_pline[2] = ON_3dPoint(points+3*i);
        if (!triple.IsLinear(tolerance)) break;
        --n;
      }
      indices[n++] = i;
    }
    *retained = n;return 1;
  } catch (const std::exception& exception) {
    set_error(error, capacity, std::string("Polyline simplification failed: ") + exception.what());
  } catch (...) {set_error(error, capacity, "Unknown polyline simplification failure");}
  return 0;
}
