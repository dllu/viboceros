#ifndef VIBOCEROS_MESH_MERGE_SELECTION_H
#define VIBOCEROS_MESH_MERGE_SELECTION_H
// A stored n-gon is one logical face. A selected singleton cannot be merged;
// retain its existing boundary instead of decomposing and rebuilding it.
namespace {
void remove_singleton_merge_components(ON_Mesh& mesh,const ON_MeshTopology& top,
                                      ON_SimpleArray<ON_COMPONENT_INDEX>& components) {
  const unsigned face_count=mesh.FaceUnsignedCount(),ngon_count=mesh.NgonUnsignedCount();
  const auto* ngons=mesh.NgonMap(true);
  std::vector<unsigned> units(face_count);
  for(unsigned face=0;face<face_count;++face)
    units[face]=(ngons && ngons[face]<ngon_count)?ngons[face]:ngon_count+face;
  std::vector<unsigned char> selected(ngon_count+face_count,0),neighbors(selected.size(),0);
  std::vector<unsigned char> vertices(top.TopVertexCount(),0),edges(top.TopEdgeCount(),0);
  for(int i=0;i<components.Count();++i) {
    const auto ci=components[i];
    if(ci.m_type==ON_COMPONENT_INDEX::mesh_face) selected[units[ci.m_index]]=1;
    else if(ci.m_type==ON_COMPONENT_INDEX::meshtop_edge) edges[ci.m_index]=1;
    else vertices[top.m_topv_map[ci.m_index]]=1;
  }
  const auto select_edge_faces=[&](int index) {
    const auto& edge=top.m_tope[index];
    for(int i=0;i<edge.m_topf_count;++i) selected[units[edge.m_topfi[i]]]=1;
  };
  // Each topology vertex is visited once, including coincident raw seam picks.
  for(int vertex=0;vertex<top.TopVertexCount();++vertex) if(vertices[vertex]) {
    const auto& value=top.m_topv[vertex];
    for(int i=0;i<value.m_tope_count;++i) select_edge_faces(value.m_topei[i]);
  }
  for(int edge=0;edge<top.TopEdgeCount();++edge) if(edges[edge]) select_edge_faces(edge);
  for(int index=0;index<top.TopEdgeCount();++index) {
    const auto& edge=top.m_tope[index];
    unsigned first=ON_UNSET_UINT_INDEX;
    bool distinct=false;
    for(int i=0;i<edge.m_topf_count;++i) {
      const unsigned unit=units[edge.m_topfi[i]];
      if(!selected[unit]) continue;
      if(first==ON_UNSET_UINT_INDEX) first=unit;
      else if(first!=unit) { distinct=true;break; }
    }
    if(distinct) for(int i=0;i<edge.m_topf_count;++i) {
      const unsigned unit=units[edge.m_topfi[i]];
      if(selected[unit]) neighbors[unit]=1;
    }
  }
  std::vector<unsigned char> keep_edges(edges.size(),0),keep_vertices(vertices.size(),0);
  for(int index=0;index<top.TopEdgeCount();++index) {
    const auto& edge=top.m_tope[index];
    for(int i=0;i<edge.m_topf_count;++i) if(neighbors[units[edge.m_topfi[i]]]) {
      keep_edges[index]=1;break;
    }
  }
  for(int vertex=0;vertex<top.TopVertexCount();++vertex) if(vertices[vertex]) {
    const auto& value=top.m_topv[vertex];
    for(int i=0;i<value.m_tope_count;++i) if(keep_edges[value.m_topei[i]]) {
      keep_vertices[vertex]=1;break;
    }
  }
  int write=0;
  for(int read=0;read<components.Count();++read) {
    const auto ci=components[read];
    const bool keep=ci.m_type==ON_COMPONENT_INDEX::mesh_face ? neighbors[units[ci.m_index]]!=0
        : ci.m_type==ON_COMPONENT_INDEX::meshtop_edge ? keep_edges[ci.m_index]!=0
        : keep_vertices[top.m_topv_map[ci.m_index]]!=0;
    if(keep) components[write++]=ci;
  }
  components.SetCount(write);
}
}
#endif
