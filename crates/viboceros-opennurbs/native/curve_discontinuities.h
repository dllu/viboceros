#include "curve_discontinuity_query.h"

extern "C" int32_t vibo_curve_discontinuities(uint32_t degree, size_t count,
    const double* controls, size_t knot_count, const double* knots,
    int32_t smooth, double cos_angle_tolerance, size_t* found, double* parameters,
    char* error, size_t capacity) {
  if(found)*found=0;
  set_error(error,capacity,"");
  try {
    if(!found || !parameters || count<2 || count>1000000 || degree==0 || degree>=count ||
       knot_count!=count+degree+1 || (smooth!=0 && smooth!=1) ||
       !std::isfinite(cos_angle_tolerance) || cos_angle_tolerance < -1. || cos_angle_tolerance > 1.) {
      set_error(error,capacity,"Invalid curve discontinuity buffers or tolerance");return 0;
    }
    vibo_begin_open_nurbs();
    ViboWriteObject source{};source.object_type=VIBO_OBJECT_NURBS_CURVE;
    source.degree_u=degree;source.control_point_count_u=count;source.rational_curve=1;
    source.coordinates=controls;source.coordinate_count=count*4;
    source.knots_u=knots;source.knot_u_count=knot_count;
    std::string message;std::unique_ptr<ON_Object> owned(geometry_for(source,message));
    const auto* curve=ON_Curve::Cast(owned.get());
    if(!curve){set_error(error,capacity,message.empty()?"Invalid curve discontinuity source":message);return 0;}
    const auto domain=curve->Domain();double start=domain[0],parameter=0.;int hint=0;
    const auto continuity=smooth?ON::continuity::Gsmooth_continuous:ON::continuity::G1_continuous;
    while(curve->GetNextDiscontinuity(continuity,start,domain[1],&parameter,&hint,nullptr,cos_angle_tolerance)) {
      if(!(parameter>start && parameter<domain[1]) || *found>=knot_count) {
        set_error(error,capacity,"Invalid curve discontinuity traversal");return 0;
      }
      parameters[(*found)++]=parameter;start=parameter;
    }
    return 1;
  }catch(const std::exception& exception){set_error(error,capacity,std::string("Curve discontinuity query failed: ")+exception.what());}
  catch(...){set_error(error,capacity,"Unknown curve discontinuity query failure");}
  return 0;
}
