#ifndef VIBOCEROS_SUBD_FACES_H
#define VIBOCEROS_SUBD_FACES_H
// Copy the native record and delete its complement: no mesh conversion or refit.
namespace vibo_subd {
inline void retain_faces(ON_SubD& subd,const size_t* indices,size_t count,const size_t* corners,size_t corner_count) {
  require(indices && count>0 && count<=component_limit,"Invalid SubD face subset");
  std::vector<const ON_SubDFace*> faces;
  auto fit=subd.FaceIterator();
  for(auto* f=fit.FirstFace();f;f=fit.NextFace())faces.push_back(f);
  std::sort(faces.begin(),faces.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  std::unordered_set<const ON_SubDFace*> retained;
  for(size_t i=0;i<count;++i) {
    require(indices[i]<faces.size(),"SubD face outside source table");
    retained.insert(faces[indices[i]]);
  }
  std::vector<const ON_SubDVertex*> vertices;
  auto vit=subd.VertexIterator();
  for(auto* v=vit.FirstVertex();v;v=vit.NextVertex())vertices.push_back(v);
  std::sort(vertices.begin(),vertices.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  require(corner_count<=component_limit && (corner_count==0 || corners),"Invalid SubD corner subset");
  std::unordered_set<unsigned> corner_ids;
  for(size_t i=0;i<corner_count;++i) {
    require(corners[i]<vertices.size(),"SubD corner outside source table");
    corner_ids.insert(vertices[corners[i]]->m_id);
  }
  ON_SimpleArray<ON_SubDComponentPtr> removed;
  for(auto* f:faces)if(retained.count(f)==0) {
    removed.Append(ON_SubDComponentPtr::Create(f));
  }
  require(subd.DeleteComponents(removed,false),"Cannot retain SubD faces");
  if(!corner_ids.empty()) {
    ON_SimpleArray<ON_SubDComponentPtr> selected;
    auto vit=subd.VertexIterator();
    for(auto* v=vit.FirstVertex();v;v=vit.NextVertex())
      if(corner_ids.count(v->m_id))
        selected.Append(ON_SubDComponentPtr::Create(v));
    subd.SetVertexTags(selected.Array(),selected.UnsignedCount(),ON_SubDVertexTag::Corner);
  }
  subd.UpdateAllTagsAndSectorCoefficients(false);subd.ClearEvaluationCache();
  check(subd);
}
}
#endif
