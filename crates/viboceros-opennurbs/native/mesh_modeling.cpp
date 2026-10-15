#include "mesh_modeling.h"
#include "viboceros_opennurbs.h"
#include "opennurbs_public.h"
#include <algorithm>
#include <cmath>
#include <cstring>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <vector>
extern "C" void vibo_begin_open_nurbs() {
  static std::once_flag once;
  std::call_once(once,[]{ON::Begin();});
}
struct ViboMeshResult {
  std::vector<double> vertices;
  std::vector<uint32_t> faces,ngon_vertices,ngon_faces;
  std::vector<uint8_t> colors;
  std::vector<ViboNgonSpan> ngons;
  size_t changed=0;
};
namespace {
void require(bool b,const char *s){if(!b)throw std::runtime_error(s);}
constexpr size_t vertex_limit=4000000,face_limit=1000000;
void import_mesh(const ViboMeshView &v,ON_Mesh &m){
  require(v.vertex_count>=3 && v.vertex_count<=vertex_limit && v.face_count>0 && v.face_count<=face_limit && v.vertices && v.faces,"Mesh resource limit or missing arrays");
  require(v.color_count==0 || (v.color_count==v.vertex_count && v.colors),"Invalid mesh colors");
  require(v.ngon_count<=v.face_count && v.ngon_vertex_count<=8*face_limit && v.ngon_face_count<=face_limit,"N-gon resource limit");
  require(!v.ngon_count || (v.ngons && v.ngon_vertices && v.ngon_faces),"Missing n-gon arrays");
  for(size_t i=0;i<v.vertex_count;++i){
    const auto *p=v.vertices+3*i;
    const ON_3dPoint point(p[0],p[1],p[2]);
    require(point.IsValid() && m.SetVertex(static_cast<int>(i),point),"Invalid mesh vertex");
    if(v.color_count){const auto *c=v.colors+4*i;m.m_C.Append(ON_Color(c[0],c[1],c[2],c[3]));}
  }
  for(size_t i=0;i<v.face_count;++i){
    ON_MeshFace f;
    for(int j=0;j<4;++j){const auto k=v.faces[4*i+j];require(k<v.vertex_count,"Invalid face index");f.vi[j]=static_cast<int>(k);}
    require(f.IsValid(static_cast<int>(v.vertex_count)),"Invalid mesh face");m.m_F.Append(f);
  }
  std::vector<bool> occupied(v.face_count,false);
  for(size_t i=0;i<v.ngon_count;++i){
    const auto &g=v.ngons[i];
    require(g.vertex_count>=3 && g.face_count>0 && g.first_vertex<=v.ngon_vertex_count && g.vertex_count<=v.ngon_vertex_count-g.first_vertex && g.first_face<=v.ngon_face_count && g.face_count<=v.ngon_face_count-g.first_face,"Invalid n-gon span");
    for(size_t k=0;k<g.vertex_count;++k)require(v.ngon_vertices[g.first_vertex+k]<v.vertex_count,"Invalid n-gon vertex index");
    for(size_t k=0;k<g.face_count;++k){const auto f=v.ngon_faces[g.first_face+k];require(f<v.face_count && !occupied[f],"Invalid or overlapping n-gon face index");occupied[f]=true;}
    require(m.AddNgon(static_cast<unsigned>(g.vertex_count),v.ngon_vertices+g.first_vertex,static_cast<unsigned>(g.face_count),v.ngon_faces+g.first_face)>=0,"Invalid input n-gon");
  }
  require(m.IsValid(),"Invalid native input mesh");
}
size_t logical_count(const ON_Mesh &m){
  size_t count=m.FaceCount();
  for(int i=0;i<m.NgonCount();++i){const auto *g=m.Ngon(i);if(g && g->m_Fcount)count-=g->m_Fcount-1;}
  return count;
}
size_t merge(ON_Mesh &m,double tolerance,double angle){
  require(std::isfinite(tolerance)&&tolerance>0&&std::isfinite(angle)&&angle>0,"Invalid coplanar merge tolerances");
  const auto &top=m.Topology();const ON_3dPointListRef points(&m);
  const size_t n=m.FaceCount();
  std::vector<ON_3dVector> normals(n);
  for(size_t f=0;f<n;++f)require(m.m_F[static_cast<int>(f)].ComputeFaceNormal(points,normals[f]),"Degenerate mesh face cannot be merged");
  std::vector<std::vector<int>> units;
  std::vector<int> owner(n,-1);
  for(int g=0;g<m.NgonCount();++g){
    const auto *ngon=m.Ngon(g);if(!ngon)continue;
    const int u=static_cast<int>(units.size());units.emplace_back();
    for(unsigned k=0;k<ngon->m_Fcount;++k){const unsigned f=ngon->m_fi[k];require(f<n && owner[f]<0,"Overlapping n-gon faces");owner[f]=u;units.back().push_back(static_cast<int>(f));}
  }
  for(size_t f=0;f<n;++f)if(owner[f]<0){owner[f]=static_cast<int>(units.size());units.push_back({static_cast<int>(f)});}
  std::vector<std::vector<int>> neighbors(units.size());
  struct Edge {int index,a,b;};std::vector<Edge> edges;
  for(int e=0;e<top.m_tope.Count();++e){
    const auto &edge=top.m_tope[e];if(edge.m_topf_count!=2)continue;
    const int f=edge.m_topfi[0],g=edge.m_topfi[1];if(f==g)continue;
    const int a=owner[f],b=owner[g];if(a==b)continue;
    bool orientation[2]={false,false};bool found[2]={false,false};
    for(int k=0;k<2;++k){const auto &face=top.m_topf[k?g:f];for(int j=0;j<(m.m_F[k?g:f].IsTriangle()?3:4);++j)if(face.m_topei[j]==e){orientation[k]=face.m_reve[j]!=0;found[k]=true;}}
    if(!found[0] || !found[1] || orientation[0]==orientation[1])continue;
    neighbors[a].push_back(b);neighbors[b].push_back(a);edges.push_back({e,a,b});
  }
  std::vector<int> group(units.size(),-1);int next_group=0;size_t work=0;
  const auto fits=[&](int unit,const ON_3dPoint &origin,const ON_3dVector &normal){
    for(int f:units[unit]){
      require(++work<=32000000,"Coplanar mesh work limit");
      const double a=std::atan2(ON_CrossProduct(normal,normals[f]).Length(),ON_DotProduct(normal,normals[f]));
      if(!std::isfinite(a)||a>angle)return false;
      const auto &face=m.m_F[f];
      for(int j=0;j<(face.IsTriangle()?3:4);++j){
        const double distance=std::abs(ON_DotProduct(normal,points[face.vi[j]]-origin));
        if(!std::isfinite(distance)||distance>tolerance)return false;
      }
    }
    return true;
  };
  for(size_t seed=0;seed<units.size();++seed){
    if(group[seed]>=0)continue;
    const int first=units[seed][0];const auto normal=normals[first];const auto origin=points[m.m_F[first].vi[0]];
    const int current_group=next_group++;
    group[seed]=current_group;
    if(!fits(static_cast<int>(seed),origin,normal))continue;
    std::vector<int> queue{static_cast<int>(seed)};
    for(size_t q=0;q<queue.size();++q)for(int other:neighbors[queue[q]]){
      if(group[other]>=0 || !fits(other,origin,normal))continue;
      group[other]=current_group;queue.push_back(other);
    }
  }
  ON_SimpleArray<ON_COMPONENT_INDEX> selected;
  for(const auto &e:edges)if(group[e.a]==group[e.b])selected.Append(ON_COMPONENT_INDEX(ON_COMPONENT_INDEX::meshtop_edge,e.index));
  if(!selected.Count())return 0;
  const size_t before=logical_count(m);
  m.MergeFaceSets(selected);
  require(m.IsValid(),"Coplanar merge produced an invalid mesh");
  const size_t after=logical_count(m);require(after<=before,"Coplanar merge increased the logical face count");
  return before-after;
}
void export_mesh(const ON_Mesh &m,ViboMeshResult &r){
  const ON_3dPointListRef points(&m);
  for(int i=0;i<m.VertexCount();++i){const auto p=points[i];r.vertices.insert(r.vertices.end(),{p.x,p.y,p.z});}
  for(int f=0;f<m.FaceCount();++f)for(int j=0;j<4;++j)r.faces.push_back(static_cast<uint32_t>(m.m_F[f].vi[j]));
  if(m.HasVertexColors())for(int i=0;i<m.VertexCount();++i){const auto c=m.m_C[i];r.colors.insert(r.colors.end(),{static_cast<uint8_t>(c.Red()),static_cast<uint8_t>(c.Green()),static_cast<uint8_t>(c.Blue()),static_cast<uint8_t>(c.Alpha())});}
  for(int i=0;i<m.NgonCount();++i){const auto *g=m.Ngon(i);if(!g)continue;
    r.ngons.push_back({r.ngon_vertices.size(),g->m_Vcount,r.ngon_faces.size(),g->m_Fcount});
    r.ngon_vertices.insert(r.ngon_vertices.end(),g->m_vi,g->m_vi+g->m_Vcount);r.ngon_faces.insert(r.ngon_faces.end(),g->m_fi,g->m_fi+g->m_Fcount);
  }
}
}
extern "C" int vibo_mesh_edit(const ViboMeshView *input,uint32_t operation,double distance,double angle,ViboMeshResult **out,char *error,size_t capacity){
  if(out)*out=nullptr;
  try{
    require(input && out,"Missing mesh arguments");vibo_begin_open_nurbs();
    ON_Mesh mesh;import_mesh(*input,mesh);auto result=std::make_unique<ViboMeshResult>();
    if(operation==1)result->changed=merge(mesh,distance,angle);
    else if(operation==2){require(std::isfinite(distance)&&distance>=0,"Invalid planar tolerance");result->changed=mesh.AddPlanarNgons(nullptr,distance,3,2,false);}
    else if(operation==3){
      require(mesh.NgonCount()==0,"Outer boundary query expects undecorated faces");
      ON_SimpleArray<unsigned> faces;for(int f=0;f<mesh.FaceCount();++f)faces.Append(static_cast<unsigned>(f));
      ON_SimpleArray<unsigned> vertices,counts;
      const auto count=ON_MeshNgon::FindNgonBoundaries(ON_3dPointListRef(&mesh),ON_MeshFaceList(&mesh),nullptr,faces.UnsignedCount(),faces.Array(),vertices,counts);
      require(count>=3 && counts.Count()>0 && counts[0]>=3 && counts[0]<=vertices.UnsignedCount(),"No valid outer boundary");
      require(mesh.AddNgon(counts[0],vertices.Array(),faces.UnsignedCount(),faces.Array())>=0,"Invalid outer boundary");
    }else throw std::runtime_error("Unknown mesh operation");
    export_mesh(mesh,*result);*out=result.release();return 1;
  }catch(const std::exception &e){if(error && capacity){std::strncpy(error,e.what(),capacity-1);error[capacity-1]=0;}return 0;}
  catch(...){if(error && capacity){std::strncpy(error,"Unknown native mesh error",capacity-1);error[capacity-1]=0;}return 0;}
}
extern "C" int vibo_mesh_result_view(const ViboMeshResult *r,ViboMeshView *v,size_t *changed){
  if(!r || !v || !changed)return 0;
  *v={r->vertices.size()/3,r->faces.size()/4,r->colors.size()/4,r->ngons.size(),r->ngon_vertices.size(),r->ngon_faces.size(),r->vertices.data(),r->faces.data(),r->colors.data(),r->ngons.data(),r->ngon_vertices.data(),r->ngon_faces.data()};*changed=r->changed;return 1;
}
extern "C" void vibo_mesh_result_free(ViboMeshResult *r){delete r;}
