// Public OpenNURBS limit frames, sharing the exact surface transfer constructor.
extern "C" int32_t vibo_surface_frames(const uint32_t* degree, const size_t* count,
    const double* controls, const size_t* knot_count, const double* knots_u,
    const double* knots_v, size_t parameter_count, const double* parameters,
    int32_t* found, double* frames, char* error, size_t capacity) {
  set_error(error, capacity, "");
  try {
    if (!degree || !count || !controls || !knot_count || !knots_u || !knots_v ||
        !parameters || !found || !frames || parameter_count > 1000000 ||
        count[0] == 0 || count[1] == 0 ||
        count[0] > std::numeric_limits<size_t>::max() / count[1] / 4) {
      set_error(error, capacity, "Invalid surface frame buffers"); return 0;
    }
    for(size_t i=0;i<parameter_count;++i) found[i]=0;
    vibo_begin_open_nurbs();
    ViboWriteObject source{};
    source.object_type=VIBO_OBJECT_NURBS_SURFACE;
    source.degree_u=degree[0];source.degree_v=degree[1];
    source.control_point_count_u=count[0];source.control_point_count_v=count[1];
    source.coordinates=controls;source.coordinate_count=count[0]*count[1]*4;
    source.knots_u=knots_u;source.knot_u_count=knot_count[0];
    source.knots_v=knots_v;source.knot_v_count=knot_count[1];
    std::string message;
    std::unique_ptr<ON_Object> owned(geometry_for(source,message));
    const ON_Surface* surface=ON_Surface::Cast(owned.get());
    if(!surface){set_error(error,capacity,message.empty()?"Surface frame transfer failed":message);return 0;}
    for(size_t i=0;i<parameter_count;++i){
      const double u=parameters[2*i],v=parameters[2*i+1];
      if(!ON_IsValid(u)||!ON_IsValid(v)||!surface->Domain(0).Includes(u)||!surface->Domain(1).Includes(v)){
        set_error(error,capacity,"Surface frame parameter is outside the domain");return 0;
      }
      ON_Plane frame;
      if(!surface->FrameAt(u,v,frame)) continue;
      found[i]=1;
      const double values[12]={frame.origin.x,frame.origin.y,frame.origin.z,
          frame.xaxis.x,frame.xaxis.y,frame.xaxis.z,frame.yaxis.x,frame.yaxis.y,frame.yaxis.z,
          frame.zaxis.x,frame.zaxis.y,frame.zaxis.z};
      std::copy(values,values+12,frames+12*i);
    }
    return 1;
  }catch(const std::exception& exception){set_error(error,capacity,std::string("Surface frame query failed: ")+exception.what());return 0;}
}
