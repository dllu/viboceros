#ifndef VIBOCEROS_SUBD_OFFSET_H
#define VIBOCEROS_SUBD_OFFSET_H
#include <unordered_set>
namespace vibo_subd {
inline ON_3dVector offset_normal(const ON_SubDVertex* v) {
  ON_3dVector normal=ON_3dVector::ZeroVector;
  std::unordered_set<const ON_SubDFace*> sectors;
  for(unsigned i=0;i<v->FaceCount();++i) {
    ON_SubDSectorSurfacePoint point=ON_SubDSectorSurfacePoint::Nan;
    require(v->GetSurfacePoint(v->Face(i),true,point),"Cannot evaluate SubD offset normal");
    if(sectors.insert(point.m_sector_face).second)normal+=ON_3dVector(point.m_limitN);
  }
  require(normal.IsValid() && normal.Unitize(),"Undefined SubD offset normal");
  return normal;
}
inline std::unique_ptr<ON_SubD> offset_subd(const uint8_t* bytes,size_t count,double distance,bool solid) {
  require(std::isfinite(distance) && distance!=0.,"Invalid SubD offset distance");
  auto output=decode(bytes,count);
  std::vector<const ON_SubDVertex*> vertices;
  std::vector<const ON_SubDEdge*> edges;
  std::vector<const ON_SubDFace*> faces;
  auto vit=output->VertexIterator();for(auto* v=vit.FirstVertex();v;v=vit.NextVertex())vertices.push_back(v);
  auto eit=output->EdgeIterator();for(auto* e=eit.FirstEdge();e;e=eit.NextEdge())edges.push_back(e);
  auto fit=output->FaceIterator();size_t corners=0;
  for(auto* f=fit.FirstFace();f;f=fit.NextFace()){faces.push_back(f);corners+=f->EdgeCount();}
  const auto order=[](auto* a,auto* b){return a->m_id<b->m_id;};
  std::sort(vertices.begin(),vertices.end(),order);std::sort(edges.begin(),edges.end(),order);std::sort(faces.begin(),faces.end(),order);
  uint64_t normal_work=0;for(auto* v:vertices){normal_work+=uint64_t(v->FaceCount())*v->FaceCount();}
  require(normal_work<=16000000,"SubD offset sector evaluation budget exceeded");
  std::unordered_set<const ON_SubDVertex*> boundary_vertices;
  size_t boundary_count=0;for(auto* e:edges){require(e->FaceCount()==1 || e->FaceCount()==2,"SubD offset requires manifold edges");if(e->FaceCount()==1){++boundary_count;boundary_vertices.insert(e->m_vertex[0]);boundary_vertices.insert(e->m_vertex[1]);}}
  const bool walls=solid && boundary_count>0;
  require(corners<=4*component_limit && (!walls || (2*vertices.size()<=component_limit && 2*edges.size()+boundary_vertices.size()<=4*component_limit && 2*faces.size()+boundary_count<=component_limit && 2*corners+4*boundary_count<=4*component_limit)),"SubD offset component budget exceeded");
  // Evaluate every normal on the unchanged private net before moving controls.
  // One decode retains native properties without another complete archive copy.
  std::vector<ON_3dPoint> original,positions;
  for(auto* v:vertices) {
    const auto p=v->ControlNetPoint();original.push_back(p);positions.push_back(p+distance*offset_normal(v));
    require(positions.back().IsValid(),"Invalid SubD offset control point");
  }
  std::unordered_map<const ON_SubDVertex*,ON_SubDVertex*> shifted,base;
  for(size_t i=0;i<vertices.size();++i) {
    auto* target=const_cast<ON_SubDVertex*>(vertices[i]);
    require(target->SetControlNetPoint(positions[i],false),"Cannot move SubD offset control");shifted.emplace(vertices[i],target);
  }
  if(walls) {
    for(size_t i=0;i<vertices.size();++i) {auto* v=vertices[i];auto* added=output->AddVertex(v->m_vertex_tag,&original[i].x);require(added,"Cannot copy SubD offset control");base.emplace(v,added);}
    std::unordered_map<const ON_SubDEdge*,ON_SubDEdge*> copied;
    for(auto* e:edges) {
      auto* added=output->AddEdge(e->m_edge_tag,base.at(e->m_vertex[0]),base.at(e->m_vertex[1]));require(added,"Cannot copy SubD offset edge");
      if(e->IsSmooth())added->SetSharpnessForExperts(e->Sharpness(false));copied.emplace(e,added);
    }
    for(auto* f:faces) {
      ON_SimpleArray<ON_SubDEdgePtr> boundary;
      for(unsigned j=0;j<f->EdgeCount();++j) {const unsigned k=f->EdgeCount()-1-j;auto* e=copied.at(f->Edge(k));auto* v=base.at(f->Vertex((k+1)%f->EdgeCount()));boundary.Append(ON_SubDEdgePtr::Create(e,e->m_vertex[0]==v?0:1));}
      auto* added=output->AddFace(boundary);require(added,"Cannot copy SubD offset face");
      added->SetPerFaceColor(f->PerFaceColor());added->SetMaterialChannelIndex(f->MaterialChannelIndex());
      if(f->TexturePointsAreSet()) {
        require(output->AllocateFaceTexturePoints(added)>=f->EdgeCount(),"Cannot copy SubD offset texture points");
        for(unsigned j=0;j<f->EdgeCount();++j)require(added->SetTexturePoint(j,f->TexturePoint((f->EdgeCount()-j)%f->EdgeCount())),"Cannot copy SubD offset texture point");
      }
    }
    std::unordered_map<const ON_SubDVertex*,ON_SubDEdge*> vertical;
    for(auto* e:edges)if(e->FaceCount()==1) {
      const auto* face=e->Face(0);const unsigned side=face->EdgeArrayIndex(e);
      const auto* a=face->Vertex(side);const auto* b=face->Vertex((side+1)%face->EdgeCount());
      auto bridge=[&](const ON_SubDVertex* v,bool reverse) {
        auto found=vertical.find(v);ON_SubDEdge* edge;
        if(found!=vertical.end())edge=found->second;
        else {auto* x=shifted.at(v);auto* y=base.at(v);if(reverse)std::swap(x,y);
          edge=output->AddEdge(v->IsCorner()?ON_SubDEdgeTag::Crease:ON_SubDEdgeTag::Smooth,x,y);require(edge,"Cannot create SubD offset wall edge");vertical.emplace(v,edge);}
        return edge;
      };
      auto* ea=bridge(a,false);auto* eb=bridge(b,true);auto* top=const_cast<ON_SubDEdge*>(e);auto* bottom=copied.at(e);
      ON_SubDEdge* wall_edges[4]={top,ea,bottom,eb};ON_SubDVertex* wall_vertices[4]={shifted.at(b),shifted.at(a),base.at(a),base.at(b)};
      ON_SimpleArray<ON_SubDEdgePtr> boundary;
      for(unsigned j=0;j<4;++j)boundary.Append(ON_SubDEdgePtr::Create(wall_edges[j],wall_edges[j]->m_vertex[0]==wall_vertices[j]?0:1));
      require(output->AddFace(boundary),"Cannot create SubD offset wall");
    }
    if(distance<0)require(output->ReverseOrientation(),"Cannot orient negative SubD shell");
  }
  output->UpdateAllTagsAndSectorCoefficients(false);output->ClearEvaluationCache();check(*output);return output;
}
}
#endif
