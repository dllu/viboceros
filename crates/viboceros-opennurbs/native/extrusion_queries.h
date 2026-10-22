// Owned extrusion construction and its canonical, smooth-face B-rep transfer.
struct ViboExtrusionResult {
  std::vector<uint8_t> archive,brep;
};
namespace {
template<class F> int extrusion_guard(ViboExtrusionResult** out,char* error,size_t capacity,F run) {
  if(out)*out=nullptr;set_error(error,capacity,"");
  try {
    vibo_extrusion::require(out,"Missing extrusion output");vibo_begin_open_nurbs();
    auto extrusion=run();vibo_extrusion::require(extrusion!=nullptr,"Cannot construct extrusion");
    auto result=std::make_unique<ViboExtrusionResult>();result->archive=vibo_extrusion::encode(*extrusion);
    std::unique_ptr<ON_Brep> brep(extrusion->BrepForm(nullptr,true));
    vibo_extrusion::require(brep && brep->IsValid() && brep->m_F.Count()<=100000 && brep->m_E.Count()<=1000000 && brep->m_V.Count()<=1000000,"Invalid extrusion B-rep or topology budget exceeded");
    BridgeObject transfer;vibo_extrusion::require(append_brep(*brep,transfer),"Cannot transfer extrusion B-rep");
    vibo_extrusion::require(transfer.geometry_data.size()<=256U*1024U*1024U,"Extrusion B-rep transfer budget exceeded");
    result->brep=std::move(transfer.geometry_data);*out=result.release();return 1;
  }catch(const std::exception& exception){set_error(error,capacity,exception.what());}
  catch(...){set_error(error,capacity,"Unknown extrusion failure");}
  return 0;
}
}
extern "C" int vibo_extrusion_read(const uint8_t* bytes,size_t count,ViboExtrusionResult** out,char* error,size_t capacity) {
  return extrusion_guard(out,error,capacity,[&]{return vibo_extrusion::decode(bytes,count);});
}
extern "C" int vibo_extrusion_transform(const uint8_t* bytes,size_t count,const double* matrix,ViboExtrusionResult** out,char* error,size_t capacity) {
  return extrusion_guard(out,error,capacity,[&]{
    vibo_extrusion::require(matrix,"Missing extrusion transform");auto extrusion=vibo_extrusion::decode(bytes,count);
    ON_Xform transform;for(int i=0;i<4;++i)for(int j=0;j<4;++j){vibo_extrusion::require(ON_IsValid(matrix[4*i+j]),"Invalid extrusion transform");transform[i][j]=matrix[4*i+j];}
    vibo_extrusion::require(transform.IsAffine() && transform.Determinant()!=0. && extrusion->Transform(transform),"Cannot represent transformed extrusion");return extrusion;
  });
}
extern "C" int vibo_extrusion_create(uint32_t degree,size_t count,const double* controls,
    size_t knot_count,const double* knots,const double* frame,double height,int cap,
    ViboExtrusionResult** out,char* error,size_t capacity) {
  return extrusion_guard(out,error,capacity,[&]{
    vibo_extrusion::require(frame && count>1 && count<=100000 && ON_IsValid(height) && height!=0. && (cap==0 || cap==1),"Invalid extrusion construction input");
    for(int i=0;i<9;++i)vibo_extrusion::require(ON_IsValid(frame[i]),"Invalid extrusion profile plane");
    ViboWriteObject source{};source.object_type=VIBO_OBJECT_NURBS_CURVE;source.degree_u=degree;
    source.control_point_count_u=count;source.coordinates=controls;source.coordinate_count=4*count;source.knots_u=knots;source.knot_u_count=knot_count;
    std::string message;std::unique_ptr<ON_Object> owned(geometry_for(source,message));
    const auto* curve=ON_Curve::Cast(owned.get());vibo_extrusion::require(curve,"Invalid extrusion profile");
    ON_Plane plane(ON_3dPoint(frame[0],frame[1],frame[2]),ON_3dVector(frame[3],frame[4],frame[5]),ON_3dVector(frame[6],frame[7],frame[8]));
    vibo_extrusion::require(plane.IsValid(),"Invalid extrusion profile plane");
    return std::unique_ptr<ON_Extrusion>(ON_Extrusion::CreateFrom3dCurve(*curve,&plane,height,cap!=0,nullptr));
  });
}
extern "C" int vibo_extrusion_view(const ViboExtrusionResult* result,const uint8_t** archive,size_t* archive_count,const uint8_t** brep,size_t* brep_count) {
  if(!result || !archive || !archive_count || !brep || !brep_count)return 0;
  *archive=result->archive.data();*archive_count=result->archive.size();*brep=result->brep.data();*brep_count=result->brep.size();return 1;
}
extern "C" void vibo_extrusion_free(ViboExtrusionResult* result) {delete result;}
