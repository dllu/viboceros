#ifndef VIBOCEROS_SUBD_FACE_PROPERTIES_H
#define VIBOCEROS_SUBD_FACE_PROPERTIES_H
extern "C" int vibo_subd_set_face_properties(const uint8_t* bytes,size_t count,
    const ViboSubDFaceProperties* input,size_t input_count,
    ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&] {
    require(input_count<=vibo_subd::component_limit && (input_count==0 || input),"Invalid SubD face-property edits");
    auto subd=vibo_subd::decode(bytes,count);
    std::vector<const ON_SubDFace*> faces;auto fit=subd->FaceIterator();
    for(auto* f=fit.FirstFace();f;f=fit.NextFace()) faces.push_back(f);
    std::sort(faces.begin(),faces.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
    std::unordered_set<unsigned> seen;
    for(size_t i=0;i<input_count;++i)
      require(input[i].face<faces.size() && input[i].material<=65535 && seen.insert(input[i].face).second,
        "Invalid or duplicate SubD face-property target");
    for(size_t i=0;i<input_count;++i) {
      const auto& value=input[i];const auto* f=faces[value.face];
      if(value.color==0xFFFFFFFFU) f->ClearPerFaceColor();
      else {
        ON_Color color;color.SetRGBA(value.color&255,(value.color>>8)&255,(value.color>>16)&255,(value.color>>24)&255);
        f->SetPerFaceColor(color);
      }
      f->SetMaterialChannelIndex(static_cast<int>(value.material));
    }
    return extract(*subd);
  });
}
#endif
