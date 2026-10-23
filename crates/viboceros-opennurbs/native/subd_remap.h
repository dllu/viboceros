#ifndef VIBOCEROS_SUBD_REMAP_H
#define VIBOCEROS_SUBD_REMAP_H
#include <tuple>
namespace vibo_subd {
// Rust supplies corner assignments; public expert tools retain IDs and face data.
inline size_t remap_vertices(ON_SubD& subd,const size_t* copies,size_t copy_count,
    const size_t* retired,size_t retired_count,const size_t* representatives,size_t edge_count,
    const uint32_t* corners,size_t corner_count,bool weld) {
  require(copy_count<=component_limit && retired_count<=component_limit &&
    edge_count==subd.EdgeCount() && corner_count<=8*component_limit &&
    (copy_count==0 || copies) && (retired_count==0 || retired) && representatives && corners,
    "Invalid or excessive SubD topology remap");
  std::vector<ON_SubDVertex*> vertices;auto vit=subd.VertexIterator();
  for(auto* v=vit.FirstVertex();v;v=vit.NextVertex()) vertices.push_back(const_cast<ON_SubDVertex*>(v));
  std::sort(vertices.begin(),vertices.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  const size_t old_count=vertices.size();
  require(copy_count<=component_limit-old_count,"SubD cut vertex limit exceeded");
  std::vector<bool> boundary(old_count),copied(old_count);
  for(size_t i=0;i<old_count;++i) boundary[i]=vertices[i]->EdgeProperties().m_boundary_edge_count>0;
  for(size_t i=0;i<copy_count;++i) {require(copies[i]<old_count,"Invalid copied SubD vertex");copied[copies[i]]=true;}
  std::set<size_t> removed;
  for(size_t i=0;i<retired_count;++i) require(retired[i]<old_count && removed.insert(retired[i]).second,"Invalid retired SubD vertex");
  std::vector<ON_SubDEdge*> edges;auto eit=subd.EdgeIterator();
  for(auto* e=eit.FirstEdge();e;e=eit.NextEdge()) edges.push_back(const_cast<ON_SubDEdge*>(e));
  std::sort(edges.begin(),edges.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  std::unordered_map<const ON_SubDEdge*,size_t> edge_index;
  for(size_t i=0;i<edges.size();++i) {
    require(representatives[i]<edge_count && representatives[representatives[i]]==representatives[i],"Invalid SubD edge equivalence");
    require(edges[i]->FaceCount()>0 && edges[i]->FaceCount()<=2,"SubD seam remapping requires manifold face edges");
    edge_index.emplace(edges[i],i);
  }
  std::vector<ON_SubDFace*> faces;auto fit=subd.FaceIterator();
  for(auto* f=fit.FirstFace();f;f=fit.NextFace()) faces.push_back(const_cast<ON_SubDFace*>(f));
  std::sort(faces.begin(),faces.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  using Key=std::tuple<size_t,uint32_t,uint32_t>;
  struct Group {ON_SubDEdge* edge=nullptr;uint32_t a=0,b=0;size_t uses=0,sources=0;};
  std::map<Key,Group> groups;
  std::vector<std::vector<Key>> boundaries(faces.size());
  std::vector<std::vector<uint32_t>> face_vertices(faces.size());
  std::vector<Key> preferred(edges.size());std::vector<bool> has_preferred(edges.size());
  size_t offset=0;
  for(size_t f=0;f<faces.size();++f) {
    const size_t count=faces[f]->EdgeCount();
    require(offset<=corner_count && count<=corner_count-offset,"Missing remapped SubD corners");
    std::set<uint32_t> distinct;
    for(size_t j=0;j<count;++j) {
      const uint32_t v=corners[offset+j];
      require(v<old_count+copy_count && !removed.count(v) && distinct.insert(v).second,"Invalid or collapsed remapped SubD face");
      face_vertices[f].push_back(v);
    }
    for(size_t j=0;j<count;++j) {
      const auto ptr=faces[f]->EdgePtr(static_cast<unsigned>(j));
      const size_t e=edge_index.at(ptr.Edge()),rep=representatives[e];
      const uint32_t a=face_vertices[f][j],b=face_vertices[f][(j+1)%count];
      const Key key{rep,std::min(a,b),std::max(a,b)};auto& group=groups[key];
      if(group.uses++==0) {group.a=ptr.EdgeDirection()?b:a;group.b=ptr.EdgeDirection()?a:b;}
      require(group.uses<=2,"Remapped SubD edge would be nonmanifold");
      if(edges[e]->Face(0)==faces[f]) ++group.sources;
      if(e==rep && edges[e]->Face(0)==faces[f]) {
        preferred[rep]=key;has_preferred[rep]=true;
        group.a=ptr.EdgeDirection()?b:a;group.b=ptr.EdgeDirection()?a:b;
      }
      boundaries[f].push_back(key);
    }
    offset+=count;
  }
  require(offset==corner_count && groups.size()<=4*component_limit,"Excessive remapped SubD edges");
  for(size_t i=0;i<copy_count;++i) {
    auto* source=vertices[copies[i]];const auto p=source->ControlNetPoint();
    const auto tag=boundary[copies[i]] || source->IsCorner()?ON_SubDVertexTag::Corner:ON_SubDVertexTag::Unset;
    auto* vertex=subd.AddVertex(tag,&p.x);require(vertex && vertex->m_id>vertices.back()->m_id,"Cannot append cut SubD vertex ID");vertices.push_back(vertex);
  }
  for(size_t i=0;i<old_count;++i) if(copied[i] && boundary[i]) vertices[i]->m_vertex_tag=ON_SubDVertexTag::Corner;
  for(auto* face:faces) require(subd.RemoveFaceConnections(face),"Cannot detach SubD face boundary");
  for(size_t i=0;i<edges.size();++i) if(representatives[i]!=i) {
    require(subd.RemoveEdgeVertexConnection(edges[i],0U) && subd.RemoveEdgeVertexConnection(edges[i],1U),"Cannot detach welded SubD edge");
    require(subd.ReturnEdgeForExperts(edges[i]),"Cannot retire welded SubD edge");
  }
  std::vector<size_t> group_counts(edges.size());
  for(const auto& item:groups) ++group_counts[std::get<0>(item.first)];
  size_t changed=0;
  for(auto& item:groups) {
    const size_t rep=std::get<0>(item.first);auto& group=item.second;
    const bool merged=weld && group.sources>1,split=!weld && group_counts[rep]>1;
    auto* a=vertices[group.a];auto* b=vertices[group.b];
    if(has_preferred[rep] && preferred[rep]==item.first) {
      auto* edge=edges[rep];
      for(unsigned end=0;end<2;++end) {
        auto* v=end==0?a:b;if(edge->m_vertex[end]==v) continue;
        require(subd.RemoveEdgeVertexConnection(edge,end)!=nullptr && subd.GrowVertexEdgeArray(v,v->m_edge_count+1),"Cannot reattach SubD edge endpoint");
        edge->m_vertex[end]=v;v->m_edges[v->m_edge_count++]=ON_SubDEdgePtr::Create(edge,end);
      }
      group.edge=edge;
    } else {
      require(split,"Missing original SubD edge representative");
      group.edge=subd.AddEdge(ON_SubDEdgeTag::Crease,a,b);require(group.edge && group.edge->m_id>edges.back()->m_id,"Cannot append cut SubD edge ID");++changed;
    }
    if(merged || split) {
      group.edge->m_edge_tag=merged?ON_SubDEdgeTag::Smooth:ON_SubDEdgeTag::Crease;group.edge->ClearSharpnessForExperts();
      for(auto* v:{a,b}) if(merged || !v->IsCorner()) v->m_vertex_tag=ON_SubDVertexTag::Unset;
      if(merged) changed+=group.sources-1;
    }
  }
  for(size_t i=0;i<faces.size();++i) {
    ON_SimpleArray<ON_SubDEdgePtr> boundary_edges;
    for(size_t j=0;j<boundaries[i].size();++j) {
      auto* e=groups.at(boundaries[i][j]).edge;
      boundary_edges.Append(ON_SubDEdgePtr::Create(e,e->m_vertex[0]==vertices[face_vertices[i][j]]?0:1));
    }
    require(subd.SetFaceBoundary(faces[i],boundary_edges),"Cannot restore remapped SubD face");
  }
  for(size_t i:removed) require(subd.ReturnVertexForExperts(vertices[i]),"Welded SubD vertex retains connections");
  subd.UpdateAllTagsAndSectorCoefficients(false);subd.ClearEvaluationCache();return changed;
}
}
#endif
