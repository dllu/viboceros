#ifndef VIBOCEROS_MESH_CODEC_H
#define VIBOCEROS_MESH_CODEC_H
// Shared, validated mesh import for native modeling and SubD conversion.
#include "mesh_modeling.h"
#include "opennurbs_public.h"
#include <cmath>
#include <limits>
#include <stdexcept>
#include <vector>
namespace vibo_mesh {
inline void require(bool b,const char* message) { if(!b) throw std::runtime_error(message); }
constexpr size_t vertex_limit=4000000,face_limit=1000000;
inline void import_mesh(const ViboMeshView &v,ON_Mesh &m,bool repair=false){
  require(v.vertex_count>=3 && v.vertex_count<=vertex_limit && v.face_count>0 && v.face_count<=face_limit && v.vertices && v.faces,"Mesh resource limit or missing arrays");
  require(v.color_count==0 || (v.color_count==v.vertex_count && v.colors),"Invalid mesh colors");
  require(v.ngon_count<=v.face_count && v.ngon_vertex_count<=8*face_limit && v.ngon_face_count<=face_limit,"N-gon resource limit");
  require(!v.ngon_count || (v.ngons && v.ngon_vertices && v.ngon_faces),"Missing n-gon arrays");
  for(size_t i=0;i<v.vertex_count;++i){
    const auto *p=v.vertices+3*i;
    const ON_3dPoint point(p[0],p[1],p[2]);
    require(std::isfinite(p[0]) && std::isfinite(p[1]) && std::isfinite(p[2]) && (repair || point.IsValid()) && m.SetVertex(static_cast<int>(i),point),"Invalid mesh vertex");
    if(v.color_count){const auto *c=v.colors+4*i;m.m_C.Append(ON_Color(c[0],c[1],c[2],c[3]));}
  }
  for(size_t i=0;i<v.face_count;++i){
    ON_MeshFace f;
    for(int j=0;j<4;++j){const auto k=v.faces[4*i+j];require(repair || k<v.vertex_count,"Invalid face index");f.vi[j]=k<=static_cast<uint32_t>(std::numeric_limits<int>::max())?static_cast<int>(k):-1;}
    require(repair || f.IsValid(static_cast<int>(v.vertex_count)),"Invalid mesh face");m.m_F.Append(f);
  }
  std::vector<bool> occupied(v.face_count,false);
  for(size_t i=0;i<v.ngon_count;++i){
    const auto &g=v.ngons[i];
    require(g.vertex_count>=3 && g.face_count>0 && g.first_vertex<=v.ngon_vertex_count && g.vertex_count<=v.ngon_vertex_count-g.first_vertex && g.first_face<=v.ngon_face_count && g.face_count<=v.ngon_face_count-g.first_face,"Invalid n-gon span");
    for(size_t k=0;k<g.vertex_count;++k)require(v.ngon_vertices[g.first_vertex+k]<v.vertex_count,"Invalid n-gon vertex index");
    for(size_t k=0;k<g.face_count;++k){const auto f=v.ngon_faces[g.first_face+k];require(f<v.face_count && !occupied[f],"Invalid or overlapping n-gon face index");occupied[f]=true;}
    require(m.AddNgon(static_cast<unsigned>(g.vertex_count),v.ngon_vertices+g.first_vertex,static_cast<unsigned>(g.face_count),v.ngon_faces+g.first_face)>=0,"Invalid input n-gon");
  }
  require(repair || m.IsValid(),"Invalid native input mesh");
}
 }
#endif
