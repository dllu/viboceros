// Create owned SDK fixtures with explicit cached principal curvatures.
// These synthetic channels test statistics, not the mesh's geometric curvature.
#include "opennurbs_public.h"
#include <cmath>
#include <iostream>
#include <vector>

int main(int argc,char** argv) {
  if(argc!=2) return 2;
  ON::Begin();
  std::vector<ON_SurfaceCurvature> samples;
  double a,b;
  while(std::cin>>a>>b) {
    if(!std::isfinite(a) || !std::isfinite(b)) return 3;
    samples.push_back(ON_SurfaceCurvature::CreateFromPrincipalCurvatures(a,b));
  }
  if(!std::cin.eof() || samples.size()<3 || samples.size()>10000) return 4;
  ON_Mesh mesh;
  for(size_t i=0;i<samples.size();++i) {
    const double angle=2*ON_PI*i/samples.size();
    mesh.m_V.Append(ON_3fPoint(static_cast<float>(10*cos(angle)),static_cast<float>(10*sin(angle)),0));
  }
  for(int i=1;i+1<static_cast<int>(samples.size());++i) {
    ON_MeshFace face;face.vi[0]=0;face.vi[1]=i;face.vi[2]=face.vi[3]=i+1;mesh.m_F.Append(face);
  }
  mesh.ComputeVertexNormals();
  for(const auto& k:samples) mesh.m_K.Append(k);
  if(!mesh.HasPrincipalCurvatures() || !mesh.IsValid()) return 5;
  ONX_Model model;ON_3dmObjectAttributes attributes;
  if(model.AddModelGeometryComponent(&mesh,&attributes).IsEmpty()) return 6;
  if(!model.Write(argv[1],80)) return 7;
  ONX_Model reread;
  if(!reread.Read(argv[1])) return 8;
  ONX_ModelComponentIterator it(reread,ON_ModelComponent::Type::ModelGeometry);
  const auto* component=ON_ModelGeometryComponent::FromModelComponentRef(it.FirstComponentReference(),nullptr);
  const auto* saved=component?ON_Mesh::Cast(component->Geometry(nullptr)):nullptr;
  if(!saved || saved->m_K.Count()!=static_cast<int>(samples.size())) return 9;
  for(size_t i=0;i<samples.size();++i)
    if(saved->m_K[i].k1!=samples[i].k1 || saved->m_K[i].k2!=samples[i].k2) return 10;
  return 0;
}
