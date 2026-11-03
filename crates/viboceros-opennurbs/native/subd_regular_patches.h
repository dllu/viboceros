#ifndef VIBOCEROS_SUBD_REGULAR_PATCHES_H
#define VIBOCEROS_SUBD_REGULAR_PATCHES_H
// Public exact regular-patch evaluation, clamped into bicubic Bezier form.
struct ViboSubDRegularPatches {
  std::vector<size_t> faces;
  std::vector<double> controls;
};
extern "C" int vibo_subd_regular_patches(const uint8_t* bytes,size_t count,
    ViboSubDRegularPatches** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    auto subd=vibo_subd::decode(bytes,count);
    std::vector<const ON_SubDFace*> faces;
    auto it=subd->FaceIterator();
    for(auto* face=it.FirstFace();face;face=it.NextFace()) faces.push_back(face);
    std::sort(faces.begin(),faces.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
    auto result=std::make_unique<ViboSubDRegularPatches>();
    for(size_t i=0;i<faces.size();++i) {
      ON_NurbsSurface surface;
      if(!faces[i]->GetQuadSurface(surface)) continue;
      require(surface.ClampEnd(0,2) && surface.ClampEnd(1,2),"Cannot clamp exact SubD patch");
      require(surface.IsValid() && surface.Dimension()==3 && !surface.IsRational() &&
        surface.Order(0)==4 && surface.Order(1)==4 && surface.CVCount(0)==4 && surface.CVCount(1)==4 &&
        surface.Domain(0)==ON_Interval(0,1) && surface.Domain(1)==ON_Interval(0,1),
        "Invalid exact SubD patch structure");
      // Rust surfaces and native oracle records store u-varying-fastest nets.
      for(int v=0;v<4;++v) for(int u=0;u<4;++u) {
        ON_3dPoint p;require(surface.GetCV(u,v,p),"Cannot read exact SubD control");
        point(result->controls,p);
      }
      result->faces.push_back(i);
    }
    return result;
  });
}
extern "C" int vibo_subd_regular_patches_view(const ViboSubDRegularPatches* r,ViboSubDRegularPatchesView* v) {
  if(!r || !v) return 0;
  *v={r->faces.size(),r->faces.data(),r->controls.data()};return 1;
}
extern "C" void vibo_subd_regular_patches_free(ViboSubDRegularPatches* r) {delete r;}
#endif
