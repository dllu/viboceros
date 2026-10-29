#ifndef VIBOCEROS_SUBD_MERGE_H
#define VIBOCEROS_SUBD_MERGE_H
#include "subd_merge_selection.h"
namespace vibo_subd {
inline bool can_merge_pair(const ON_SubDEdgePtr a,const ON_SubDEdgePtr b,
    const std::unordered_set<unsigned>& affected,const std::unordered_set<unsigned>& original_valence_three,
    const std::unordered_set<unsigned>& corner_removal_vertices) {
  const auto* v=b.RelativeVertex(0);
  if(!v || !affected.count(v->m_id) || v->EdgeCount()!=2 || (v->IsCorner() && !corner_removal_vertices.count(v->m_id)) ||
     !ON_SubD::EdgesAreConsecutive(a,b)) return false;
  // Boundary junctions of original valence four or more survive even when
  // collinear. Interior valence-two vertices can be removed at any old valence.
  if(v->FaceCount()==1 && !original_valence_three.count(v->m_id)) return false;
  for(unsigned i=0;i<v->FaceCount();++i) if(v->Face(i)->EdgeCount()<=3) return false;
  return true;
}
inline bool merge_region(ON_SubD& subd,const std::vector<unsigned>& ids,
    const std::unordered_set<unsigned>& corner_removal_edges) {
  const std::unordered_set<unsigned> members(ids.begin(),ids.end());
  std::vector<ON_SubDFace*> faces;
  std::vector<ON_SubDEdge*> interior;
  std::unordered_set<unsigned> interior_ids,affected,original_valence_three,corner_removal_vertices;
  std::vector<ON_SubDEdgePtr> boundary;
  for(auto id:ids) {
    auto* f=const_cast<ON_SubDFace*>(subd.FaceFromId(id));require(f,"Missing SubD merge region face");faces.push_back(f);
    for(unsigned i=0;i<f->EdgeCount();++i) {
      auto ep=f->EdgePtr(i);auto* e=ep.Edge();
      if(e->FaceCount()>2) return false;
      const bool inside=e->FaceCount()==2 && members.count(e->Face(0)->m_id) && members.count(e->Face(1)->m_id);
      if(inside) {
        if(interior_ids.insert(e->m_id).second) interior.push_back(e);
        for(auto* v:e->m_vertex) {
          affected.insert(v->m_id);
          if(v->EdgeCount()==3) original_valence_three.insert(v->m_id);
          if(corner_removal_edges.count(e->m_id)) corner_removal_vertices.insert(v->m_id);
        }
      } else boundary.push_back(ep);
    }
  }
  if(boundary.size()<3 || boundary.size()>ON_SubDFace::MaximumEdgeCount) return false;
  // A SubD face has one simple boundary. Reject holes, pinches and inconsistent
  // winding before editing any topology or tags.
  std::unordered_map<const ON_SubDVertex*,ON_SubDEdgePtr> outgoing;
  for(auto ep:boundary) if(!outgoing.emplace(ep.RelativeVertex(0),ep).second) return false;
  std::vector<ON_SubDEdgePtr> ordered;ordered.reserve(boundary.size());
  auto ep=boundary.front();
  while(ordered.size()<boundary.size()) {
    ordered.push_back(ep);const auto next=outgoing.find(ep.RelativeVertex(1));
    if(next==outgoing.end()) return false;
    ep=next->second;
    if(ep.m_ptr==ordered.front().m_ptr) break;
  }
  if(ordered.size()!=boundary.size() || ep.m_ptr!=ordered.front().m_ptr) return false;
  // Public expert removal keeps the level and its ID allocator alive even
  // when every original face belongs to this region.
  for(auto* f:faces) require(subd.RemoveFaceConnections(f),"Cannot disconnect SubD merge face");
  for(auto* f:faces) require(subd.ReturnFaceForExperts(f),"Cannot release SubD merge face");
  auto* merged=subd.AddFaceForExperts(ids.front(),ordered.data(),static_cast<unsigned>(ordered.size()));
  require(merged && merged->m_id==ids.front(),"Cannot retain SubD merged face identity");
  // Release only this region's unused components. A global deletion pass
  // would scan the entire SubD for every disconnected region and could also
  // remove unrelated isolated data.
  std::unordered_set<unsigned> orphan_candidates;
  for(auto* e:interior) {
    require(e->FaceCount()==0,"SubD merge interior edge is still connected");
    for(unsigned end=0;end<2;++end) {
      auto* v=subd.RemoveEdgeVertexConnection(e,end);
      require(v,"Cannot disconnect SubD merge interior edge");orphan_candidates.insert(v->m_id);
    }
    require(subd.ReturnEdgeForExperts(e),"Cannot release SubD merge interior edge");
  }
  for(auto id:orphan_candidates) {
    auto* v=const_cast<ON_SubDVertex*>(subd.VertexFromId(id));
    if(v && v->EdgeCount()==0 && v->FaceCount()==0)
      require(subd.ReturnVertexForExperts(v),"Cannot release SubD merge interior vertex");
  }
  unsigned i=1;
  while(i<merged->EdgeCount()) {
    auto a=merged->EdgePtr(i-1),b=merged->EdgePtr(i);
    if(can_merge_pair(a,b,affected,original_valence_three,corner_removal_vertices)) {
      require(subd.MergeConsecutiveEdges(a,b).m_ptr==a.m_ptr,"Cannot simplify SubD merged boundary");
    } else ++i;
  }
  auto a=merged->EdgePtr(merged->EdgeCount()-1),b=merged->EdgePtr(0);
  if(can_merge_pair(a,b,affected,original_valence_three,corner_removal_vertices))
    require(subd.MergeConsecutiveEdges(a,b).m_ptr==a.m_ptr,"Cannot simplify SubD merged boundary seam");
  return true;
}
}
extern "C" int vibo_subd_merge_faces(const uint8_t* bytes,size_t count,
    const ViboSubDMergeComponent* input,size_t input_count,size_t* removed,
    ViboSubDResult** out,char* error,size_t capacity) {
  if(removed) *removed=0;
  return guard(out,error,capacity,[&] {
    require(removed,"Missing SubD merge count");auto subd=vibo_subd::decode(bytes,count);
    const auto selected=vibo_subd::merge_selection(*subd,input,input_count);
    std::unordered_set<unsigned> retained;auto fit=subd->FaceIterator();
    for(auto* f=fit.FirstFace();f;f=fit.NextFace()) retained.insert(f->m_id);
    const auto before=subd->FaceCount();
    for(const auto& region:selected.regions) if(vibo_subd::merge_region(*subd,region,selected.corner_removal_edges))
      for(auto id:region) retained.erase(id);
    if(subd->FaceCount()<before) {
      // Accepted explicit vertex requests also simplify surviving valence-two
      // vertices outside merged regions, but a getter with no merge stays a no-op.
      for(auto id:selected.vertices) {
        auto* v=subd->VertexFromId(id);if(!v || v->EdgeCount()!=2 || v->FaceCount()==0) continue;
        auto* f=v->Face(0);const auto i=f->VertexIndex(v);if(i>=f->EdgeCount()) continue;
        auto a=f->EdgePtr((i+f->EdgeCount()-1)%f->EdgeCount()),b=f->EdgePtr(i);
        const std::unordered_set<unsigned> request{id};
        if(!vibo_subd::can_merge_pair(a,b,request,request,request)) continue;
        for(unsigned j=0;j<v->FaceCount();++j) retained.erase(v->Face(j)->m_id);
        require(subd->MergeConsecutiveEdges(a,b).m_ptr==a.m_ptr,"Cannot simplify selected SubD vertex");
      }
      subd->UpdateAllTagsAndSectorCoefficients(false);subd->ClearEvaluationCache();
    }
    require(subd->FaceCount()<=before,"SubD merging increased face count");
    *removed=before-subd->FaceCount();return extract(*subd,false,false,nullptr,&retained);
  });
}
#endif
