#ifndef VIBOCEROS_SUBD_FACE_PACKS_H
#define VIBOCEROS_SUBD_FACE_PACKS_H
struct ViboSubDFacePacks { std::vector<ViboSubDFacePack> records; };
extern "C" int vibo_subd_face_packs(const uint8_t* bytes,size_t count,
    ViboSubDFacePacks** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    auto subd=vibo_subd::decode(bytes,count);
    std::vector<const ON_SubDFace*> faces;
    auto it=subd->FaceIterator();
    for(auto* f=it.FirstFace();f;f=it.NextFace())faces.push_back(f);
    std::sort(faces.begin(),faces.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
    auto result=std::make_unique<ViboSubDFacePacks>();result->records.reserve(faces.size());
    for(auto* f:faces) {
      ViboSubDFacePack record{};record.id=f->PackId();
      if(f->PackRectIsSet()) {
        const auto p=f->PackRectOrigin();const auto s=f->PackRectSize();
        record.set=1;record.rotation=f->PackRectRotationDegrees()/90;
        record.rectangle[0]=p.x;record.rectangle[1]=p.y;
        record.rectangle[2]=s.x;record.rectangle[3]=s.y;
      }
      result->records.push_back(record);
    }
    return result;
  });
}
extern "C" int vibo_subd_face_packs_view(const ViboSubDFacePacks* r,ViboSubDFacePacksView* v) {
  if(!r || !v)return 0;*v={r->records.size(),r->records.data()};return 1;
}
extern "C" void vibo_subd_face_packs_free(ViboSubDFacePacks* r) {delete r;}
#endif
