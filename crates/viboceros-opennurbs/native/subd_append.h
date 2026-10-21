#ifndef VIBOCEROS_SUBD_APPEND_H
#define VIBOCEROS_SUBD_APPEND_H
#include <queue>
namespace vibo_subd {
// Add to the decoded original rather than reconstructing it through a mesh.
// Existing component IDs, interior creases, sharpness and face data survive.
inline void append_polygons(ON_SubD& subd,const double* points,size_t point_count,
    const uint32_t* sizes,size_t face_count,const uint32_t* corners,size_t corner_count) {
  require(points && sizes && corners && point_count>=subd.VertexCount() &&
    point_count<=component_limit && face_count>0 && face_count<=component_limit-subd.FaceCount() &&
    corner_count<=4*component_limit,"Invalid or excessive SubD append input");
  std::vector<ON_SubDVertex*> vertices;
  std::unordered_map<const ON_SubDVertex*,uint32_t> indices;
  auto vit=subd.VertexIterator();
  for(auto* v=vit.FirstVertex();v;v=vit.NextVertex()) {
    const auto p=v->ControlNetPoint();const size_t i=vertices.size();
    require(p==ON_3dPoint(points+3*i),"SubD append cannot edit existing controls");
    indices.emplace(v,static_cast<uint32_t>(i));vertices.push_back(const_cast<ON_SubDVertex*>(v));
  }
  struct Edge {
    ON_SubDEdge* edge=nullptr;
    unsigned old_count=0;
    bool old_forward=false;
    std::vector<std::pair<size_t,bool>> uses;
  };
  std::map<std::pair<uint32_t,uint32_t>,Edge> edges;
  auto eit=subd.EdgeIterator();
  for(auto* e=eit.FirstEdge();e;e=eit.NextEdge()) {
    const auto a=indices.at(e->m_vertex[0]),b=indices.at(e->m_vertex[1]);
    auto key=std::make_pair(std::min(a,b),std::max(a,b));
    require(edges.emplace(key,Edge{const_cast<ON_SubDEdge*>(e),0,false,{}}).second,
      "Ambiguous parallel SubD edges cannot be appended");
  }
  size_t old_corners=0;auto fit=subd.FaceIterator();
  for(auto* f=fit.FirstFace();f;f=fit.NextFace()) {
    old_corners+=f->EdgeCount();
    for(unsigned i=0;i<f->EdgeCount();++i) {
      const auto a=indices.at(f->Vertex(i)),b=indices.at(f->Vertex((i+1)%f->EdgeCount()));
      auto& edge=edges.at(std::make_pair(std::min(a,b),std::max(a,b)));
      ++edge.old_count;edge.old_forward=a<b;
    }
  }
  require(old_corners<=4*component_limit-corner_count,"SubD append corner limit exceeded");
  std::vector<std::vector<uint32_t>> faces;
  size_t offset=0;
  for(size_t i=0;i<face_count;++i) {
    const size_t n=sizes[i];
    require(n>=3 && n<=ON_SubDFace::MaximumEdgeCount && offset<=corner_count && n<=corner_count-offset,
      "Invalid appended SubD face size");
    faces.emplace_back(corners+offset,corners+offset+n);
    std::set<uint32_t> distinct;
    for(size_t j=0;j<n;++j) {
      const auto a=faces.back()[j],b=faces.back()[(j+1)%n];
      require(a<point_count && b<point_count && distinct.insert(a).second,"Invalid appended SubD vertex");
      auto& edge=edges[std::make_pair(std::min(a,b),std::max(a,b))];
      edge.uses.emplace_back(i,a<b);
      require(edge.old_count+edge.uses.size()<=ON_SubDFace::MaximumEdgeCount,"SubD edge incidence limit exceeded");
    }
    offset+=n;
  }
  require(offset==corner_count,"Trailing SubD append corners");
  // Solve orientation over the entire new face set. A face encountered before
  // its eventual connecting bridge must receive the same orientation as a face
  // encountered after the bridge. Original faces are fixed anchors.
  std::vector<int> flip(face_count,-1);
  std::vector<std::vector<std::pair<size_t,bool>>> adjacent(face_count);
  std::queue<size_t> pending;
  const auto assign=[&](size_t face,int orientation) {
    require(flip[face]<0 || flip[face]==orientation,"Inconsistent SubD append orientation");
    if(flip[face]<0) {flip[face]=orientation;pending.push(face);}
  };
  for(const auto& item:edges) {
    const auto& edge=item.second;
    if(edge.old_count==1 && edge.uses.size()==1) {
      const auto [face,forward]=edge.uses[0];assign(face,forward==edge.old_forward);
    } else if(edge.old_count==0 && edge.uses.size()==2) {
      const auto [a,forward_a]=edge.uses[0];const auto [b,forward_b]=edge.uses[1];
      adjacent[a].emplace_back(b,forward_a==forward_b);
      adjacent[b].emplace_back(a,forward_a==forward_b);
    }
  }
  const auto propagate=[&] {
    while(!pending.empty()) {
      const auto face=pending.front();pending.pop();
      for(const auto& next:adjacent[face]) assign(next.first,flip[face]^next.second);
    }
  };
  propagate();
  for(size_t i=0;i<face_count;++i) if(flip[i]<0) {assign(i,0);propagate();}
  for(size_t i=vertices.size();i<point_count;++i) {
    const ON_3dPoint p(points+3*i);require(p.IsValid(),"Invalid appended SubD control");
    auto* v=subd.AddVertex(ON_SubDVertexTag::Unset,points+3*i);
    require(v,"Cannot allocate appended SubD vertex");vertices.push_back(v);
  }
  for(size_t i=0;i<faces.size();++i) {
    auto& face=faces[i];if(flip[i]) std::reverse(face.begin()+1,face.end());
    ON_SimpleArray<ON_SubDEdgePtr> boundary;
    for(size_t j=0;j<face.size();++j) {
      const auto a=face[j],b=face[(j+1)%face.size()];
      auto& entry=edges.at(std::make_pair(std::min(a,b),std::max(a,b)));
      if(!entry.edge) {entry.edge=subd.AddEdge(vertices[a],vertices[b]);require(entry.edge,"Cannot allocate appended SubD edge");}
      else if(entry.edge->FaceCount()>=1) {
        entry.edge->m_edge_tag=ON_SubDEdgeTag::Unset;
        // The native face command retags the endpoints of the joined boundary,
        // including a former corner, while leaving unrelated tags in place.
        vertices[a]->m_vertex_tag=ON_SubDVertexTag::Unset;
        vertices[b]->m_vertex_tag=ON_SubDVertexTag::Unset;
      }
      boundary.Append(ON_SubDEdgePtr::Create(entry.edge,entry.edge->m_vertex[0]==vertices[a]?0:1));
    }
    require(subd.AddFace(boundary),"Cannot append SubD face");
  }
  subd.UpdateAllTagsAndSectorCoefficients(false);
  subd.ClearEvaluationCache();
}
}
#endif
