#ifndef VIBOCEROS_SUBD_FACE_PACKS_H
#define VIBOCEROS_SUBD_FACE_PACKS_H
struct ViboSubDFacePacks { std::vector<ViboSubDFacePack> records; };
// Small, project-owned metadata setter compiled beside the pinned OpenNURBS
// source. This stamps our own algorithm ID rather than claiming native packing.
void vibo_subd_stamp_face_packs(ON_SubD&);
extern "C" int vibo_subd_set_face_packs(const uint8_t* bytes,size_t count,
    const ViboSubDFacePack* records,size_t record_count,ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    auto subd=vibo_subd::decode(bytes,count);
    require(records && record_count==subd->FaceCount(),"Incomplete SubD face packing");
    std::vector<ON_SubDFace*> faces;auto it=subd->FaceIterator();
    for(auto* f=it.FirstFace();f;f=it.NextFace())faces.push_back(const_cast<ON_SubDFace*>(f));
    std::sort(faces.begin(),faces.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
    for(size_t i=0;i<faces.size();++i) {
      const auto& r=records[i];
      require(r.id>0 && r.set==1 && r.rotation<4,"Invalid SubD face-pack identity or rotation");
      require(faces[i]->SetPackRectForExperts(ON_2dPoint(r.rectangle[0],r.rectangle[1]),
        ON_2dVector(r.rectangle[2],r.rectangle[3]),90*static_cast<int>(r.rotation)),"Invalid SubD face-pack rectangle");
      faces[i]->SetPackIdForExperts(r.id);
    }
    subd->ClearEvaluationCache();subd->ChangeRenderContentSerialNumber();
    vibo_subd_stamp_face_packs(*subd);
    return extract(*subd);
  });
}
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
