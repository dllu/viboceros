#ifndef VIBOCEROS_SUBD_MERGE_SELECTION_H
#define VIBOCEROS_SUBD_MERGE_SELECTION_H
#include <numeric>
namespace vibo_subd {
struct MergeSelection {
  std::vector<std::vector<unsigned>> regions;
  std::unordered_set<unsigned> corner_removal_edges;
  std::vector<unsigned> vertices;
};
inline MergeSelection merge_selection(ON_SubD& subd,const ViboSubDMergeComponent* input,size_t count) {
  require(count<=4*component_limit && (count==0 || input),"Invalid SubD merge selection");
  std::vector<const ON_SubDVertex*> vertices;
  auto vit=subd.VertexIterator();
  for(auto* v=vit.FirstVertex();v;v=vit.NextVertex()) vertices.push_back(v);
  std::sort(vertices.begin(),vertices.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  std::vector<const ON_SubDFace*> faces;
  auto fit=subd.FaceIterator();
  for(auto* f=fit.FirstFace();f;f=fit.NextFace()) faces.push_back(f);
  std::sort(faces.begin(),faces.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  std::unordered_map<const ON_SubDFace*,size_t> face_index;
  for(size_t i=0;i<faces.size();++i) face_index.emplace(faces[i],i);
  std::map<std::pair<unsigned,unsigned>,const ON_SubDEdge*> edges;
  auto eit=subd.EdgeIterator();
  for(auto* e=eit.FirstEdge();e;e=eit.NextEdge()) {
    const auto key=std::minmax(e->m_vertex[0]->m_id,e->m_vertex[1]->m_id);
    require(edges.emplace(key,e).second,"Ambiguous parallel SubD merge edges");
  }
  std::unordered_set<const ON_SubDFace*> selected_faces;
  std::unordered_set<const ON_SubDEdge*> selected_edges;
  std::unordered_set<const ON_SubDVertex*> selected_vertices;
  MergeSelection selection;
  for(size_t i=0;i<count;++i) {
    const auto& c=input[i];
    if(c.kind==0) {
      require(c.first<vertices.size(),"SubD merge vertex outside source table");
      selected_vertices.insert(vertices[c.first]);
    } else if(c.kind==1) {
      require(c.first<vertices.size() && c.second<vertices.size() && c.first!=c.second,
        "SubD merge edge endpoints outside source table");
      const auto found=edges.find(std::minmax(vertices[c.first]->m_id,vertices[c.second]->m_id));
      require(found!=edges.end(),"SubD merge endpoints are not an edge");
      selected_edges.insert(found->second);
    } else if(c.kind==2) {
      require(c.first<faces.size(),"SubD merge face outside source table");
      selected_faces.insert(faces[c.first]);
    } else require(false,"Unknown SubD merge component");
  }
  // Resolve overlapping requests in the same face/edge/vertex priority order
  // observed in SubD and documented by public mesh MergeFaceSets. An ignored
  // edge must not consume an independent vertex request.
  auto blocked_vertices=selected_faces;
  std::unordered_set<const ON_SubDEdge*> requested_edges;
  for(auto* e:selected_edges) if(e->FaceCount()==2 && !selected_faces.count(e->Face(0)) && !selected_faces.count(e->Face(1))) {
    requested_edges.insert(e);blocked_vertices.insert(e->Face(0));blocked_vertices.insert(e->Face(1));
  }
  for(auto* v:selected_vertices) {
    bool accepted=true;
    for(unsigned i=0;i<v->FaceCount();++i) if(blocked_vertices.count(v->Face(i))) accepted=false;
    if(accepted) {
      selection.vertices.push_back(v->m_id);
      for(unsigned i=0;i<v->EdgeCount();++i) {
        auto* e=v->Edge(i);requested_edges.insert(e);selection.corner_removal_edges.insert(e->m_id);
      }
    }
  }
  std::sort(selection.vertices.begin(),selection.vertices.end());
  for(const auto& item:edges) {
    const auto* e=item.second;
    if(e->FaceCount()==2 && selected_faces.count(e->Face(0)) && selected_faces.count(e->Face(1))) {
      requested_edges.insert(e);selection.corner_removal_edges.insert(e->m_id);
    }
  }
  std::vector<size_t> parent(faces.size());std::iota(parent.begin(),parent.end(),0);
  const auto root=[&](size_t i) {
    while(parent[i]!=i) {parent[i]=parent[parent[i]];i=parent[i];}return i;
  };
  for(auto* e:requested_edges) if(e->FaceCount()==2) {
    auto a=root(face_index.at(e->Face(0))),b=root(face_index.at(e->Face(1)));
    if(a!=b) parent[std::max(a,b)]=std::min(a,b);
  }
  std::map<size_t,std::vector<unsigned>> regions;
  for(size_t i=0;i<faces.size();++i) regions[root(i)].push_back(faces[i]->m_id);
  for(auto& item:regions) if(item.second.size()>1) selection.regions.push_back(std::move(item.second));
  return selection;
}
}
#endif
