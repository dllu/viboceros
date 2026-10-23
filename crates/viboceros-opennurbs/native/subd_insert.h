#ifndef VIBOCEROS_SUBD_INSERT_H
#define VIBOCEROS_SUBD_INSERT_H
namespace vibo_subd {
template<class Component> inline std::vector<Component*> active_components(ON_SubD& subd);
template<> inline std::vector<ON_SubDVertex*> active_components(ON_SubD& subd) {
  std::vector<ON_SubDVertex*> rows;auto it=subd.VertexIterator();
  for(auto* v=it.FirstVertex();v;v=it.NextVertex()) rows.push_back(const_cast<ON_SubDVertex*>(v));
  std::sort(rows.begin(),rows.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});return rows;
}
template<> inline std::vector<ON_SubDEdge*> active_components(ON_SubD& subd) {
  std::vector<ON_SubDEdge*> rows;auto it=subd.EdgeIterator();
  for(auto* e=it.FirstEdge();e;e=it.NextEdge()) rows.push_back(const_cast<ON_SubDEdge*>(e));
  std::sort(rows.begin(),rows.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});return rows;
}
template<> inline std::vector<ON_SubDFace*> active_components(ON_SubD& subd) {
  std::vector<ON_SubDFace*> rows;auto it=subd.FaceIterator();
  for(auto* f=it.FirstFace();f;f=it.NextFace()) rows.push_back(const_cast<ON_SubDFace*>(f));
  std::sort(rows.begin(),rows.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});return rows;
}
template<class Component> inline size_t ordinal(ON_SubD& subd,const Component* value) {
  auto rows=active_components<Component>(subd);auto found=std::find(rows.begin(),rows.end(),value);
  require(found!=rows.end(),"Missing inserted SubD component");return static_cast<size_t>(found-rows.begin());
}
}
extern "C" int vibo_subd_insert_vertex(const uint8_t* bytes,size_t count,size_t index,
    const double* point,size_t* vertex,size_t* new_edge,int* changed,
    ViboSubDResult** out,char* error,size_t capacity) {
  if(changed)*changed=0;
  return guard(out,error,capacity,[&] {
    require(point && vertex && new_edge && changed,"Missing SubD split edge data");
    const ON_3dPoint location(point[0],point[1],point[2]);require(location.IsValid(),"Invalid SubD inserted point");
    auto subd=vibo_subd::decode(bytes,count);auto edges=vibo_subd::active_components<ON_SubDEdge>(*subd);
    require(index<edges.size() && subd->VertexCount()<vibo_subd::component_limit,"Invalid or excessive SubD edge split");
    auto* source=edges[index];
    if(location==source->m_vertex[0]->ControlNetPoint() || location==source->m_vertex[1]->ControlNetPoint())return extract(*subd);
    if(const auto* edge=subd->SplitEdge(source,location)) {
      subd->UpdateAllTagsAndSectorCoefficients(false);subd->ClearEvaluationCache();
      *vertex=vibo_subd::ordinal(*subd,edge->m_vertex[0]);*new_edge=vibo_subd::ordinal(*subd,edge);*changed=1;
    }
    return extract(*subd);
  });
}
extern "C" int vibo_subd_insert_face_edge(const uint8_t* bytes,size_t count,size_t face,
    size_t a,size_t b,size_t* new_edge,int* changed,ViboSubDResult** out,char* error,size_t capacity) {
  if(changed)*changed=0;
  return guard(out,error,capacity,[&] {
    require(new_edge && changed,"Missing SubD split face data");
    auto subd=vibo_subd::decode(bytes,count);auto faces=vibo_subd::active_components<ON_SubDFace>(*subd);auto vertices=vibo_subd::active_components<ON_SubDVertex>(*subd);
    require(face<faces.size() && a<vertices.size() && b<vertices.size() && subd->FaceCount()<vibo_subd::component_limit,"Invalid or excessive SubD face split");
    unsigned ai=ON_UNSET_UINT_INDEX, bi=ON_UNSET_UINT_INDEX;
    for(unsigned i=0;i<faces[face]->EdgeCount();++i) {
      if(faces[face]->Vertex(i)==vertices[a])ai=i;
      if(faces[face]->Vertex(i)==vertices[b])bi=i;
    }
    require(ai!=ON_UNSET_UINT_INDEX && bi!=ON_UNSET_UINT_INDEX,"Inserted edge ends are not face vertices");
    if(const auto* edge=subd->SplitFace(faces[face],ai,bi)) {
      subd->UpdateAllTagsAndSectorCoefficients(false);subd->ClearEvaluationCache();*new_edge=vibo_subd::ordinal(*subd,edge);*changed=1;
    }
    return extract(*subd);
  });
}
extern "C" int vibo_subd_insert_path(const uint8_t* bytes,size_t count,
    const size_t* edge_indices,const double* parameters,const double* locations,size_t split_count,
    const uint32_t* chords,size_t chord_count,ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&] {
    require((split_count==0 || (edge_indices && parameters && locations)) && (chord_count==0 || chords),"Missing SubD insertion path data");
    auto subd=vibo_subd::decode(bytes,count);
    auto edges=vibo_subd::active_components<ON_SubDEdge>(*subd);
    auto vertices=vibo_subd::active_components<ON_SubDVertex>(*subd);
    require(split_count<=vibo_subd::component_limit-vertices.size() && chord_count<=vibo_subd::component_limit-subd->FaceCount(),"Excessive SubD insertion path");
    struct Segment {const ON_SubDEdge* edge;double first,last;};
    std::unordered_map<size_t,std::vector<Segment>> segments;
    vertices.reserve(vertices.size()+split_count);
    for(size_t i=0;i<split_count;++i) {
      const auto index=edge_indices[i];const double parameter=parameters[i];
      require(index<edges.size() && std::isfinite(parameter) && parameter>0. && parameter<1.,"Invalid SubD insertion edge parameter");
      auto found=segments.find(index);
      if(found==segments.end())found=segments.emplace(index,std::vector<Segment>{{edges[index],0.,1.}}).first;
      auto& parts=found->second;
      auto part=std::find_if(parts.begin(),parts.end(),[&](const auto& s){return parameter>s.first && parameter<s.last;});
      require(part!=parts.end(),"Repeated SubD insertion edge point");
      const ON_3dPoint point(locations[3*i],locations[3*i+1],locations[3*i+2]);
      require(point.IsValid(),"Invalid SubD insertion location");
      const auto* added=subd->SplitEdge(const_cast<ON_SubDEdge*>(part->edge),point);
      require(added!=nullptr,"SubD insertion edge split failed");
      vertices.push_back(const_cast<ON_SubDVertex*>(added->m_vertex[0]));
      const double last=part->last;part->last=parameter;
      parts.push_back({added,parameter,last});
    }
    for(size_t i=0;i<chord_count;++i) {
      const auto a=chords[2*i],b=chords[2*i+1];
      require(a<vertices.size() && b<vertices.size() && a!=b,"Invalid SubD insertion chord");
      const auto* v=vertices[a];const auto* w=vertices[b];bool found=false;
      for(unsigned j=0;j<v->EdgeCount();++j) {
        const auto* edge=v->Edge(j);
        if(edge && ((edge->m_vertex[0]==v && edge->m_vertex[1]==w) || (edge->m_vertex[1]==v && edge->m_vertex[0]==w))) {found=true;break;}
      }
      if(found)continue;
      for(unsigned j=0;j<v->FaceCount();++j) {
        auto* face=const_cast<ON_SubDFace*>(v->Face(j));
        unsigned ai=ON_UNSET_UINT_INDEX,bi=ON_UNSET_UINT_INDEX;
        for(unsigned k=0;k<face->EdgeCount();++k) {
          if(face->Vertex(k)==v)ai=k;
          if(face->Vertex(k)==w)bi=k;
        }
        if(ai==ON_UNSET_UINT_INDEX || bi==ON_UNSET_UINT_INDEX)continue;
        require(subd->SplitFace(face,ai,bi)!=nullptr,"SubD insertion face split failed");found=true;break;
      }
      require(found,"SubD insertion chord has no incident face");
    }
    subd->UpdateAllTagsAndSectorCoefficients(false);subd->ClearEvaluationCache();
    return extract(*subd);
  });
}
#endif
