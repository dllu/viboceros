#ifndef VIBOCEROS_SUBD_CREASE_H
#define VIBOCEROS_SUBD_CREASE_H
// Component ordinals follow the immutable archive view's increasing native IDs.
namespace vibo_subd {
inline size_t crease_components(ON_SubD& subd, const size_t* edge_indices,
    size_t edge_count, const size_t* vertex_indices, size_t vertex_count, bool crease) {
  require(edge_count <= 4*component_limit && vertex_count <= component_limit &&
    (edge_count==0 || edge_indices) && (vertex_count==0 || vertex_indices),
    "Invalid or excessive crease component selection");
  std::vector<const ON_SubDEdge*> edges;
  auto eit=subd.EdgeIterator();
  for(auto* e=eit.FirstEdge();e;e=eit.NextEdge()) edges.push_back(e);
  std::sort(edges.begin(),edges.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  std::vector<const ON_SubDVertex*> vertices;
  auto vit=subd.VertexIterator();
  for(auto* v=vit.FirstVertex();v;v=vit.NextVertex()) vertices.push_back(v);
  std::sort(vertices.begin(),vertices.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  ON_SimpleArray<ON_SubDComponentPtr> selected_edges, selected_vertices, boundary_vertices;
  for(size_t i=0;i<edge_count;++i) {
    require(edge_indices[i]<edges.size(),"Crease edge outside source table");
    selected_edges.Append(ON_SubDComponentPtr::Create(edges[edge_indices[i]]));
  }
  for(size_t i=0;i<vertex_count;++i)
    require(vertex_indices[i]<vertices.size(),"Crease vertex outside source table");

  size_t changed=0;
  std::unordered_set<const ON_SubDVertex*> consumed;
  if(!crease) {
    // An edited explicit edge consumes requests on its endpoint vertices.
    // Boundary and already-smooth edges do not consume a vertex request.
    for(size_t i=0;i<edge_count;++i) {
      const auto* e=edges[edge_indices[i]];
      if(e->FaceCount()==2 && !e->IsSmoothNotSharp()) {
        consumed.insert(e->m_vertex[0]);consumed.insert(e->m_vertex[1]);
      }
    }
    changed+=subd.SetEdgeTags(selected_edges.Array(),selected_edges.UnsignedCount(),ON_SubDEdgeTag::Smooth);
  }
  for(size_t i=0;i<vertex_count;++i) {
    auto* v=vertices[vertex_indices[i]];
    if(consumed.count(v)) continue;
    const auto component=ON_SubDComponentPtr::Create(v);
    const auto ep=v->EdgeProperties();
    if(!crease && ep.m_boundary_edge_count+ep.m_wire_edge_count==2)
      boundary_vertices.Append(component);
    else selected_vertices.Append(component);
  }
  if(!crease) {
    ON_SimpleArray<ON_SubDComponentPtr> incident;
    auto append=[&](const ON_SimpleArray<ON_SubDComponentPtr>& selected) {
      for(int i=0;i<selected.Count();++i) {
        const auto* v=selected[i].Vertex();
        // Rhino 8.32 ignores already-smooth vertices, including soft weights.
        if(v->IsSmooth()) continue;
        for(unsigned j=0;j<v->EdgeCount();++j)
          incident.Append(ON_SubDComponentPtr::Create(v->Edge(j)));
      }
    };
    append(selected_vertices);append(boundary_vertices);
    // Reclassify both ends of relaxed edges, including neighboring corners
    // that lose their third crease. Vertex setters alone retain those corners.
    changed+=subd.SetEdgeTags(incident.Array(),incident.UnsignedCount(),ON_SubDEdgeTag::Smooth);
  }
  changed+=subd.SetVertexTags(selected_vertices.Array(),selected_vertices.UnsignedCount(),
    crease ? ON_SubDVertexTag::Corner : ON_SubDVertexTag::Smooth);
  changed+=subd.SetVertexTags(boundary_vertices.Array(),boundary_vertices.UnsignedCount(),
    ON_SubDVertexTag::Crease);
  if(crease) changed+=subd.SetEdgeTags(selected_edges.Array(),selected_edges.UnsignedCount(),ON_SubDEdgeTag::Crease);
  if(changed) {
    subd.UpdateAllTagsAndSectorCoefficients(false);
    subd.ClearEvaluationCache();
  }
  return changed;
}
}
#endif
