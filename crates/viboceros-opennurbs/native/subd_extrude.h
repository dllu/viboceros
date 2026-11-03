#ifndef VIBOCEROS_SUBD_EXTRUDE_H
#define VIBOCEROS_SUBD_EXTRUDE_H
#include <unordered_set>
namespace vibo_subd {
// The public topology operation retains component IDs and native properties.
// Directions are evaluated on the unchanged private control net first.
inline std::unique_ptr<ON_SubD> extrude_subd(const uint8_t* bytes,size_t count,
    const size_t* face_indices,size_t face_count,const size_t* edge_indices,size_t edge_count,
    bool whole,int mode,double distance,const double* direction) {
  require(std::isfinite(distance) && distance!=0. && mode>=0 && mode<=2,"Invalid SubD extrusion direction or distance");
  require(face_count<=component_limit && edge_count<=4*component_limit && (face_count==0 || face_indices) && (edge_count==0 || edge_indices),"Invalid SubD extrusion selection");
  auto subd=decode(bytes,count);
  std::vector<const ON_SubDVertex*> vertices;std::vector<const ON_SubDEdge*> edges;std::vector<const ON_SubDFace*> faces;
  auto vit=subd->VertexIterator();for(auto* v=vit.FirstVertex();v;v=vit.NextVertex())vertices.push_back(v);
  auto eit=subd->EdgeIterator();for(auto* e=eit.FirstEdge();e;e=eit.NextEdge())edges.push_back(e);
  auto fit=subd->FaceIterator();size_t corners=0;for(auto* f=fit.FirstFace();f;f=fit.NextFace()){faces.push_back(f);corners+=f->EdgeCount();}
  const auto order=[](auto* a,auto* b){return a->m_id<b->m_id;};
  std::sort(vertices.begin(),vertices.end(),order);std::sort(edges.begin(),edges.end(),order);std::sort(faces.begin(),faces.end(),order);
  std::unordered_set<const ON_SubDFace*> selected_faces;std::unordered_set<const ON_SubDEdge*> selected_edges;
  require(!whole || (face_count==0 && edge_count==0),"Conflicting SubD extrusion selections");
  if(whole)selected_faces.insert(faces.begin(),faces.end());
  for(size_t i=0;i<face_count;++i){require(face_indices[i]<faces.size(),"SubD extrusion face index out of range");selected_faces.insert(faces[face_indices[i]]);}
  for(size_t i=0;i<edge_count;++i){require(edge_indices[i]<edges.size(),"SubD extrusion edge index out of range");auto* e=edges[edge_indices[i]];require(e->FaceCount()==1,"SubD extrusion requires naked selected edges");selected_edges.insert(e);}
  require(!selected_faces.empty() || !selected_edges.empty(),"Empty SubD extrusion selection");
  require(mode!=2 || selected_faces.empty(),"Outward extrusion requires naked edges alone");
  std::unordered_set<const ON_SubDVertex*> touched,boundary;
  size_t extra_faces=0;std::unordered_map<const ON_SubDVertex*,unsigned> boundary_degree;bool junction=false;
  for(auto* f:selected_faces)for(unsigned j=0;j<f->EdgeCount();++j)touched.insert(f->Vertex(j));
  for(auto* e:edges) {
    require(e->FaceCount()==1 || e->FaceCount()==2,"SubD extrusion requires manifold edges");
    unsigned selected=0;for(unsigned j=0;j<e->FaceCount();++j)selected+=selected_faces.count(e->Face(j));
    if(selected_edges.count(e)){touched.insert(e->m_vertex[0]);touched.insert(e->m_vertex[1]);}
    if(selected_edges.count(e) || (selected>0 && (e->FaceCount()==1 || selected<e->FaceCount()))) {
      ++extra_faces;for(auto* v:e->m_vertex){boundary.insert(v);if(++boundary_degree[v]>=3)junction=true;}
    }
  }
  require(corners<=4*component_limit && (junction || (vertices.size()+boundary.size()<=component_limit && faces.size()+extra_faces<=component_limit && edges.size()+extra_faces+boundary.size()<=4*component_limit && corners+4*extra_faces<=4*component_limit)),"SubD extrusion component budget exceeded");
  std::unordered_map<const ON_SubDVertex*,ON_3dVector> sums;std::unordered_map<const ON_SubDVertex*,unsigned> weights;
  if(mode==0) {
    require(direction,"Missing SubD extrusion vector");ON_3dVector normal(direction);require(normal.IsValid() && normal.Unitize(),"Invalid SubD extrusion vector");
    for(auto* v:touched){sums[v]=normal;weights[v]=1;}
  } else {
    ON_Mesh mesh;require(subd->GetControlNetMesh(&mesh,ON_SubDGetControlNetMeshPriority::Geometry) && mesh.HasVertexNormals(),"Cannot evaluate SubD extrusion normals");
    require(mesh.ComputeFaceNormals(),"Cannot evaluate SubD extrusion face normals");
    size_t mesh_face=0;
    std::unordered_map<const ON_SubDVertex*,std::unordered_set<int>> sectors;
    for(auto* f:faces) {
      const unsigned n=f->EdgeCount();const unsigned facets=n>4?n:1;
      require(mesh_face+facets<=static_cast<size_t>(mesh.FaceCount()),"Incomplete SubD extrusion face mapping");
      if(selected_faces.count(f))for(unsigned j=0;j<n;++j) {
        auto* v=f->Vertex(j);const auto& mf=mesh.m_F[static_cast<int>(mesh_face+(n>4?j:0))];const int index=mf.vi[n>4?0:j];
        if(sectors[v].insert(index).second) {
          ON_3dVector normal(mesh.m_N[index]);require(normal.Unitize(),"Undefined SubD extrusion vertex normal");
          if(weights[v]++==0)sums[v]=normal;else sums[v]+=normal;
        }
      }
      for(unsigned j=0;j<n;++j)if(selected_edges.count(f->Edge(j))) {
        ON_3dVector normal(mesh.m_FN[static_cast<int>(mesh_face+(n>4?j:0))]);require(normal.Unitize(),"Undefined SubD extrusion edge normal");
        if(mode==2){ON_3dVector tangent=f->Vertex((j+1)%n)->ControlNetPoint()-f->Vertex(j)->ControlNetPoint();require(tangent.Unitize(),"Degenerate SubD extrusion edge");normal=ON_CrossProduct(tangent,normal);}
        for(auto* v:{f->Vertex(j),f->Vertex((j+1)%n)}){if(weights[v]++==0)sums[v]=normal;else sums[v]+=normal;}
      }
      mesh_face+=facets;
    }
  }
  std::vector<ON_3dPoint> positions;positions.reserve(vertices.size());
  for(auto* v:vertices) {
    auto p=v->ControlNetPoint();if(touched.count(v) && (!junction || mode!=0)){require(weights[v]>0,"Missing SubD extrusion direction");p+=distance*(sums.at(v)/static_cast<double>(weights.at(v)));}
    require(p.IsValid(),"Invalid SubD extrusion control point");positions.push_back(p);
  }
  ON_SimpleArray<ON_SubDComponentPtr> components;
  for(auto* f:faces)if(selected_faces.count(f))components.Append(ON_SubDComponentPtr::Create(f));
  for(auto* e:edges)if(selected_edges.count(e))components.Append(ON_SubDComponentPtr::Create(e));
  // A temporary translation builds the native walls; original IDs are the moved
  // controls, and appended stationary copies retain the original positions.
  const auto transform=ON_Xform::TranslationTransformation(ON_3dVector(0.,0.,distance));
  // Rhino reports Success when a branching extrusion cannot build manifold
  // walls. WCS leaves the net alone; UVN still applies its control directions.
  // Detect that case before the public operation attempts any topology edits.
  if(!junction)require(subd->ExtrudeComponents(transform,components.Array(),components.UnsignedCount(),true,false)>0,"SubD extrusion failed");
  for(size_t i=0;i<vertices.size();++i)require(const_cast<ON_SubDVertex*>(vertices[i])->SetControlNetPoint(positions[i],false),"Cannot move SubD extrusion control");
  if(!junction)subd->ClearInactiveLevels();subd->ClearEvaluationCache();check(*subd);return subd;
}
}
#endif
