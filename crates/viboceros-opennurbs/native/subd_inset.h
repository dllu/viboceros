#ifndef VIBOCEROS_SUBD_INSET_H
#define VIBOCEROS_SUBD_INSET_H
#include <unordered_set>
namespace vibo_subd {
inline ON_3dVector inset_corner(ON_3dVector a,ON_3dVector b,double distance) {
  require(a.Unitize() && b.Unitize(),"Degenerate SubD inset boundary edge");
  const double sine=ON_CrossProduct(a,b).Length();require(sine>0.,"Undefined SubD inset corner");
  return (distance/sine)*(a+b);
}
inline ON_3dVector inset_bisector(ON_3dVector a,ON_3dVector b,ON_3dVector inside,double distance) {
  require(a.Unitize() && b.Unitize(),"Degenerate SubD inset boundary edge");
  const double sine_half=.5*(a-b).Length();
  require(sine_half>0. && inside.Unitize(),"Undefined SubD inset corner direction");
  return (distance/sine_half)*inside;
}
inline std::unique_ptr<ON_SubD> inset_subd(const uint8_t* bytes,size_t count,
    const size_t* indices,size_t index_count,double distance,bool group,bool& changed) {
  changed=false;
  require(indices && index_count>0 && index_count<=component_limit && std::isfinite(distance) && distance>0.,"Invalid SubD inset distance or faces");
  auto subd=decode(bytes,count);
  std::vector<const ON_SubDVertex*> vertices;std::vector<const ON_SubDEdge*> edges;std::vector<const ON_SubDFace*> faces;
  auto vit=subd->VertexIterator();for(auto* v=vit.FirstVertex();v;v=vit.NextVertex())vertices.push_back(v);
  auto eit=subd->EdgeIterator();for(auto* e=eit.FirstEdge();e;e=eit.NextEdge())edges.push_back(e);
  auto fit=subd->FaceIterator();size_t corners=0;for(auto* f=fit.FirstFace();f;f=fit.NextFace()){faces.push_back(f);corners+=f->EdgeCount();}
  const auto order=[](auto* a,auto* b){return a->m_id<b->m_id;};
  std::sort(vertices.begin(),vertices.end(),order);std::sort(edges.begin(),edges.end(),order);std::sort(faces.begin(),faces.end(),order);
  std::unordered_set<const ON_SubDFace*> selected;
  for(size_t i=0;i<index_count;++i){require(indices[i]<faces.size(),"SubD inset face index out of range");selected.insert(faces[indices[i]]);}
  group=group && selected.size()>1;
  require(corners<=4*component_limit,"SubD inset corner budget exceeded");
  for(auto* e:edges)require(e->FaceCount()==1 || e->FaceCount()==2,"SubD inset requires manifold face edges");
  if(group) {
    std::unordered_map<const ON_SubDVertex*,std::vector<const ON_SubDVertex*>> boundary;
    std::unordered_map<const ON_SubDVertex*,ON_3dVector> internal;
    size_t walls=0;
    for(auto* e:edges) {
      unsigned n=0;for(unsigned j=0;j<e->FaceCount();++j)n+=selected.count(e->Face(j));
      if(n==0)continue;
      if(n==2)for(unsigned j=0;j<2;++j) {
        ON_3dVector delta=e->m_vertex[1-j]->ControlNetPoint()-e->m_vertex[j]->ControlNetPoint();require(delta.Unitize(),"Degenerate SubD inset interior edge");
        auto found=internal.find(e->m_vertex[j]);if(found==internal.end())internal.emplace(e->m_vertex[j],delta);else found->second+=delta;
      }
      else {++walls;for(unsigned j=0;j<2;++j)boundary[e->m_vertex[j]].push_back(e->m_vertex[1-j]);}
    }
    // The native command succeeds unchanged when a boundary branches or when
    // all faces of a closed component were selected.
    for(const auto& entry:boundary)if(entry.second.size()!=2)return subd;
    if(walls==0)return subd;
    require(vertices.size()+boundary.size()<=component_limit && faces.size()+walls<=component_limit && edges.size()+walls+boundary.size()<=4*component_limit && corners+4*walls<=4*component_limit,"SubD inset component budget exceeded");
    std::unordered_map<const ON_SubDVertex*,ON_3dPoint> positions;
    for(const auto& entry:boundary) {
      auto* v=entry.first;const auto p=v->ControlNetPoint();auto a=entry.second[0]->ControlNetPoint()-p,b=entry.second[1]->ControlNetPoint()-p;
      auto found=internal.find(v);ON_3dVector inside;
      if(found!=internal.end())inside=found->second;
      else {auto ua=a,ub=b;require(ua.Unitize() && ub.Unitize(),"Degenerate SubD inset corner");inside=ua+ub;}
      const auto result=p+(found!=internal.end()?inset_bisector(a,b,inside,distance):inset_corner(a,b,distance));require(result.IsValid(),"Invalid SubD inset control point");positions.emplace(v,result);
    }
    ON_SimpleArray<ON_SubDComponentPtr> components;
    for(auto* f:faces)if(selected.count(f))components.Append(ON_SubDComponentPtr::Create(f));
    // Public OpenNURBS recognizes this expert inset transform in
    // Internal_IsInsetHack. It creates the original-ID caps without moving
    // controls or updating tags; the pinned public source defines the sentinel.
    ON_Xform transform=ON_Xform::Zero4x4;transform[0][2]=ON_ZERO_TOLERANCE;
    require(subd->ExtrudeComponents(transform,components.Array(),components.UnsignedCount(),true,false)>0,"SubD grouped inset failed");
    for(const auto& entry:positions)require(const_cast<ON_SubDVertex*>(entry.first)->SetControlNetPoint(entry.second,false),"Cannot move grouped inset control");
  } else {
    size_t added=0;uint64_t work=0;
    for(auto* f:faces)if(selected.count(f)){added+=f->EdgeCount();for(unsigned j=0;j<f->EdgeCount();++j)work+=f->Vertex(j)->FaceCount();}
    require(work<=16000000,"SubD inset connection work budget exceeded");
    require(vertices.size()+added<=component_limit && faces.size()+added<=component_limit && edges.size()+2*added<=4*component_limit && corners+4*added<=4*component_limit,"SubD inset component budget exceeded");
    for(auto* source:faces)if(selected.count(source)) {
      auto* face=const_cast<ON_SubDFace*>(source);const unsigned n=face->EdgeCount();
      std::vector<const ON_SubDVertex*> outer;std::vector<ON_SubDEdgePtr> original;
      for(unsigned j=0;j<n;++j){outer.push_back(face->Vertex(j));original.push_back(face->EdgePtr(j));}
      std::vector<ON_SubDVertex*> inner;std::vector<ON_SubDEdge*> bridges,rim;
      for(unsigned j=0;j<n;++j) {
        const auto p=outer[j]->ControlNetPoint();auto a=outer[(j+n-1)%n]->ControlNetPoint()-p,b=outer[(j+1)%n]->ControlNetPoint()-p;
        auto ua=a,ub=b;require(ua.Unitize() && ub.Unitize(),"Degenerate SubD inset corner");
        const auto q=p+inset_corner(a,b,distance);require(q.IsValid(),"Invalid SubD inset control point");
        auto* v=subd->AddVertex(ON_SubDVertexTag::Smooth,&q.x);require(v,"Cannot create SubD inset control");inner.push_back(v);
      }
      require(subd->RemoveFaceConnections(face),"Cannot detach inset face boundary");
      for(unsigned j=0;j<n;++j) {
        auto* bridge=subd->AddEdge(ON_SubDEdgeTag::Smooth,const_cast<ON_SubDVertex*>(outer[j]),inner[j]);require(bridge,"Cannot create SubD inset bridge");bridges.push_back(bridge);
        auto* edge=subd->AddEdge(ON_SubDEdgeTag::Smooth,inner[j],inner[(j+1)%n]);require(edge,"Cannot create SubD inset rim");rim.push_back(edge);
      }
      ON_SimpleArray<ON_SubDEdgePtr> cap;
      for(auto* e:rim)cap.Append(ON_SubDEdgePtr::Create(e,0));
      require(subd->SetFaceBoundary(face,cap),"Cannot set SubD inset cap");
      for(unsigned j=0;j<n;++j) {
        ON_SubDEdgePtr boundary[4]={original[j],ON_SubDEdgePtr::Create(bridges[(j+1)%n],0),ON_SubDEdgePtr::Create(rim[j],1),ON_SubDEdgePtr::Create(bridges[j],1)};
        require(subd->AddFace(boundary,4),"Cannot create SubD inset wall");
      }
    }
  }
  subd->UpdateAllTagsAndSectorCoefficients(true);subd->ClearInactiveLevels();subd->ClearEvaluationCache();check(*subd);changed=true;return subd;
}
}
#endif
