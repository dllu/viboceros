#ifndef VIBOCEROS_SUBD_SOFT_CREASE_H
#define VIBOCEROS_SUBD_SOFT_CREASE_H
// Public OpenNURBS tag setters maintain dependent corners and sector tags.
// Its sharpness conversion owns percentage sanitization and float storage.
namespace vibo_subd {
inline size_t soft_crease(ON_SubD& subd, const size_t* indices,
    const double* weights, size_t count, bool stored=false) {
  require(count<=4*component_limit && (count==0 || (indices && weights)),
    "Invalid or excessive soft crease input");
  std::vector<const ON_SubDEdge*> edges;
  auto eit=subd.EdgeIterator();
  for(auto* e=eit.FirstEdge();e;e=eit.NextEdge()) edges.push_back(e);
  std::sort(edges.begin(),edges.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  ON_SimpleArray<ON_SubDComponentPtr> selected;
  std::vector<ON_SubDEdgeSharpness> sharpness;
  std::unordered_set<size_t> distinct;
  // Validate the entire request before changing even the private decoded copy.
  for(size_t i=0;i<count;++i) {
    require(indices[i]<edges.size(),"Soft crease edge outside source table");
    require(distinct.insert(indices[i]).second,"Repeated soft crease edge");
    auto* e=edges[indices[i]];
    require(e->FaceCount()==2,"Soft creases require interior SubD edges");
    double ends[2];
    for(unsigned k=0;k<2;++k) {
      const double value=weights[2*i+k];
      if(stored) ends[k]=stored_sharpness_input(value);
      else {
        require(std::isfinite(value) && value>=0. && value<=100.,
          "Soft crease weights must be between 0 and 100");
        ends[k]=ON_SubDEdgeSharpness::SharpnessFromSliderValue(
          ON_Interval(0.,100.),value,ON_DBL_QNAN);
      }
    }
    const auto value=ON_SubDEdgeSharpness::FromInterval(ends[0],ends[1]);
    require(value.IsValid(),"Invalid soft crease sharpness");
    if(stored) require(value.EndSharpness(0)==weights[2*i] && value.EndSharpness(1)==weights[2*i+1],
      "Cannot preserve stored soft crease sharpness");
    sharpness.push_back(value);
    selected.Append(ON_SubDComponentPtr::Create(e));
  }
  struct EdgeState { ON_SubDEdgeTag tag; ON_SubDEdgeSharpness sharpness; };
  std::vector<EdgeState> before;
  for(auto* e:edges) before.push_back({e->m_edge_tag,e->Sharpness(false)});
  std::vector<std::pair<const ON_SubDVertex*,ON_SubDVertexTag>> vertices;
  auto vit=subd.VertexIterator();
  for(auto* v=vit.FirstVertex();v;v=vit.NextVertex()) vertices.emplace_back(v,v->m_vertex_tag);
  subd.SetEdgeTags(selected.Array(),selected.UnsignedCount(),ON_SubDEdgeTag::Smooth);
  for(size_t i=0;i<count;++i)
    const_cast<ON_SubDEdge*>(edges[indices[i]])->SetSharpnessForExperts(sharpness[i]);
  subd.UpdateAllTagsAndSectorCoefficients(false);
  subd.ClearEvaluationCache();
  // Count final semantic changes. Setting a weight back to its original value
  // must not create history merely because transient native caches changed.
  size_t changed=0;
  for(size_t i=0;i<edges.size();++i)
    if(edges[i]->m_edge_tag!=before[i].tag || edges[i]->Sharpness(false)!=before[i].sharpness) ++changed;
  for(const auto& v:vertices) if(v.first->m_vertex_tag!=v.second) ++changed;
  return changed;
}
}
#endif
