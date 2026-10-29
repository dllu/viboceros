#ifndef VIBOCEROS_SUBD_TEXTURE_POINTS_H
#define VIBOCEROS_SUBD_TEXTURE_POINTS_H
extern "C" int vibo_subd_set_face_texture_points(const uint8_t* bytes,size_t count,
    const ViboSubDFaceTexturePoints* input,size_t input_count,
    ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&] {
    require(input_count<=vibo_subd::component_limit && (input_count==0 || input),"Invalid SubD texture edits");
    auto subd=vibo_subd::decode(bytes,count);
    std::vector<const ON_SubDFace*> faces;auto fit=subd->FaceIterator();
    for(auto* f=fit.FirstFace();f;f=fit.NextFace()) faces.push_back(f);
    std::sort(faces.begin(),faces.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
    std::unordered_set<unsigned> seen;size_t total=0;bool set=false;
    for(size_t i=0;i<input_count;++i) {
      const auto& edit=input[i];
      require(edit.face<faces.size() && seen.insert(edit.face).second,"Invalid or duplicate texture target");
      require(edit.count==0 || (edit.count==faces[edit.face]->EdgeCount() && edit.points),"Texture point count must match face corners");
      total+=edit.count;require(total<=4*vibo_subd::component_limit,"Excessive texture points");
      for(unsigned j=0;j<edit.count;++j) require(ON_3dPoint(edit.points+3*j).IsValid(),"Invalid texture point");
    }
    for(size_t i=0;i<input_count;++i) {
      const auto& edit=input[i];const auto* face=faces[edit.face];
      if(edit.count==0) face->ClearTexturePoints();
      else {
        std::vector<ON_3dPoint> points;points.reserve(edit.count);
        for(unsigned j=0;j<edit.count;++j) points.emplace_back(edit.points+3*j);
        require(subd->AddFaceTexturePoints(face,points.data(),points.size()),"Cannot set face texture points");
        set=true;
      }
    }
    if(set) subd->SetTextureCoordinateType(ON_SubDTextureCoordinateType::FromFaceTexturePoints);
    subd->ClearEvaluationCache();return extract(*subd);
  });
}
#endif
