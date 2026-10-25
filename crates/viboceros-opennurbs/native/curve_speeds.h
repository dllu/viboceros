// Native derivative magnitudes retain public OpenNURBS evaluation rounding.
extern "C" int32_t vibo_curve_speeds(uint32_t degree, size_t count,
    const double* controls, size_t knot_count, const double* knots,
    size_t parameter_count, const double* parameters, double* speeds,
    char* error, size_t capacity) {
  set_error(error, capacity, "");
  try {
    if (!controls || !knots || count < 2 || count > 1000000 || degree == 0 ||
        degree > 64 || degree >= count || knot_count != count + degree + 1 ||
        parameter_count > 1000000 || (parameter_count && (!parameters || !speeds))) {
      set_error(error, capacity, "Invalid curve speed buffers"); return 0;
    }
    vibo_begin_open_nurbs();
    ViboWriteObject source{};
    source.object_type=VIBO_OBJECT_NURBS_CURVE;source.degree_u=degree;
    source.control_point_count_u=count;source.rational_curve=1;
    source.coordinates=controls;source.coordinate_count=count*4;
    source.knots_u=knots;source.knot_u_count=knot_count;
    std::string message;
    std::unique_ptr<ON_Object> owned(geometry_for(source,message));
    const auto* curve=ON_Curve::Cast(owned.get());
    if (!curve) {
      set_error(error,capacity,message.empty()?"Invalid curve speed source":message);return 0;
    }
    const auto domain=curve->Domain();
    for(size_t i=0;i<parameter_count;++i) {
      ON_3dPoint point;ON_3dVector derivative;
      if(!ON_IsValid(parameters[i]) || !domain.Includes(parameters[i]) ||
         !curve->Ev1Der(parameters[i],point,derivative)) {
        set_error(error,capacity,"Curve speed evaluation failed");return 0;
      }
      speeds[i]=derivative.Length();
      if(!derivative.IsValid() || !ON_IsValid(speeds[i]) || speeds[i]<0.) {
        set_error(error,capacity,"Invalid native curve speed");return 0;
      }
    }
    return 1;
  }catch(const std::exception& exception){set_error(error,capacity,exception.what());}
  catch(...){set_error(error,capacity,"Unknown curve speed query failure");}
  return 0;
}
