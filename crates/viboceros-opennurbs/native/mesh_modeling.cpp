#include "mesh_modeling.h"
#include "mesh_codec.h"
#include "viboceros_opennurbs.h"
#include "opennurbs_public.h"
#include <algorithm>
#include <cmath>
#include <cstring>
#include <memory>
#include <limits>
#include <mutex>
#include <stdexcept>
#include <vector>
extern "C" void vibo_begin_open_nurbs() {
  static std::once_flag once;
  std::call_once(once,[]{ON::Begin();});
}
struct ViboMeshResult {
  std::vector<double> vertices;
  std::vector<double> texture_coordinates,vertex_normals;
  std::vector<uint32_t> faces,ngon_vertices,ngon_faces;
  std::vector<uint8_t> colors;
  std::vector<ViboNgonSpan> ngons;
  size_t changed=0,removed_faces=0,repaired_faces=0,removed_vertices=0;
};
namespace {
void require(bool b,const char *s){if(!b)throw std::runtime_error(s);}
constexpr size_t vertex_limit=4000000,face_limit=1000000;
size_t logical_count(const ON_Mesh &m){
  size_t count=m.FaceCount();
  for(int i=0;i<m.NgonCount();++i){const auto *g=m.Ngon(i);if(g && g->m_Fcount)count-=g->m_Fcount-1;}
  return count;
}
size_t merge(ON_Mesh &m,double tolerance,double angle,const size_t *selected=nullptr,size_t selected_count=0,bool region=false){
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
  if(selected){
    require(selected_count==(region?1U:2U),"Select two faces or one region seed");
    for(size_t k=0;k<selected_count;++k)require(selected[k]<n,"Selected mesh face index is out of range");
    const int seed=owner[selected[0]],other=region?-1:owner[selected[1]];
    require(region || seed!=other,"Select distinct logical mesh faces");
    // Every member facet identifies the same logical n-gon and fixed plane.
    const int first=units[seed][0];const auto normal=normals[first];const auto origin=points[m.m_F[first].vi[0]];
    if(!fits(seed,origin,normal))return 0;
    group[seed]=0;std::vector<int> queue{seed};
    for(size_t q=0;q<queue.size();++q)for(int next:neighbors[queue[q]]){
      if(group[next]>=0 || (!region && next!=other) || !fits(next,origin,normal))continue;
      group[next]=0;queue.push_back(next);
    }
  }else{
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
  }
  ON_SimpleArray<ON_COMPONENT_INDEX> selected_edges;
  for(const auto &e:edges)if(group[e.a]>=0 && group[e.a]==group[e.b])selected_edges.Append(ON_COMPONENT_INDEX(ON_COMPONENT_INDEX::meshtop_edge,e.index));
  if(!selected_edges.Count())return 0;
  const size_t before=logical_count(m);
  m.MergeFaceSets(selected_edges);
  require(m.IsValid(),"Coplanar merge produced an invalid mesh");
  const size_t after=logical_count(m);require(after<=before,"Coplanar merge increased the logical face count");
  return before-after;
}
// Shared native outer-loop classification retains inner loops in the face set.
int add_face_region_ngon(ON_Mesh &mesh,const ON_SimpleArray<unsigned> &faces){
  ON_SimpleArray<unsigned> vertices,counts;
  const auto count=ON_MeshNgon::FindNgonBoundaries(ON_3dPointListRef(&mesh),ON_MeshFaceList(&mesh),nullptr,faces.UnsignedCount(),faces.Array(),vertices,counts);
  if(count<3 || counts.Count()==0 || counts[0]<3 || counts[0]>vertices.UnsignedCount())return -1;
  return mesh.AddNgon(counts[0],vertices.Array(),faces.UnsignedCount(),faces.Array());
}
#include "mesh_cleanup.h"
void export_mesh(const ON_Mesh &m,ViboMeshResult &r){
  const ON_3dPointListRef points(&m);
  for(int i=0;i<m.VertexCount();++i){const auto p=points[i];r.vertices.insert(r.vertices.end(),{p.x,p.y,p.z});}
  for(int f=0;f<m.FaceCount();++f)for(int j=0;j<4;++j)r.faces.push_back(static_cast<uint32_t>(m.m_F[f].vi[j]));
  if(m.HasVertexColors())for(int i=0;i<m.VertexCount();++i){const auto c=m.m_C[i];r.colors.insert(r.colors.end(),{static_cast<uint8_t>(c.Red()),static_cast<uint8_t>(c.Green()),static_cast<uint8_t>(c.Blue()),static_cast<uint8_t>(c.Alpha())});}
  if(m.HasTextureCoordinates())for(int i=0;i<m.VertexCount();++i){const auto t=m.m_T[i];r.texture_coordinates.insert(r.texture_coordinates.end(),{t.x,t.y});}
  if(m.HasVertexNormals())for(int i=0;i<m.VertexCount();++i){const auto n=m.m_N[i];r.vertex_normals.insert(r.vertex_normals.end(),{n.x,n.y,n.z});}
  for(int i=0;i<m.NgonCount();++i){const auto *g=m.Ngon(i);if(!g)continue;
    r.ngons.push_back({r.ngon_vertices.size(),g->m_Vcount,r.ngon_faces.size(),g->m_Fcount});
    r.ngon_vertices.insert(r.ngon_vertices.end(),g->m_vi,g->m_vi+g->m_Vcount);r.ngon_faces.insert(r.ngon_faces.end(),g->m_fi,g->m_fi+g->m_Fcount);
  }
}
}
ViboMeshResult* vibo_mesh_result_from_mesh(const ON_Mesh& mesh) {
  auto result = std::make_unique<ViboMeshResult>();
  export_mesh(mesh, *result);
  return result.release();
}
static int edit_mesh(const ViboMeshView *input,uint32_t operation,double distance,double angle,ViboMeshResult **out,char *error,size_t capacity,const size_t *selected=nullptr,size_t selected_count=0,bool region=false){
  if(out)*out=nullptr;
  try{
    require(input && out,"Missing mesh arguments");vibo_begin_open_nurbs();
    ON_Mesh mesh;vibo_mesh::import_mesh(*input,mesh,operation==4,operation==6);auto result=std::make_unique<ViboMeshResult>();
    if(operation==1)result->changed=merge(mesh,distance,angle,selected,selected_count,region);
    else if(operation==2){require(std::isfinite(distance)&&distance>=0,"Invalid planar tolerance");result->changed=mesh.AddPlanarNgons(nullptr,distance,3,2,false);}
    else if(operation==3){
      require(mesh.NgonCount()==0,"Outer boundary query expects undecorated faces");
      ON_SimpleArray<unsigned> faces;for(int f=0;f<mesh.FaceCount();++f)faces.Append(static_cast<unsigned>(f));
      require(add_face_region_ngon(mesh,faces)>=0,"No valid outer boundary");
    }else if(operation==4)cull_degenerate_faces(mesh,*result);
    else if(operation==6){mesh.ComputeFaceNormals();mesh.ComputeVertexNormals();result->changed=1;}
    else if(operation==7){
      require(std::isfinite(angle)&&std::isfinite(distance)&&distance>=0.,"Invalid quad conversion parameters");
      const int faces=mesh.FaceCount(),triangles=mesh.TriangleCount(),quads=mesh.QuadCount();
      const bool changed=mesh.ConvertTrianglesToQuads(angle,distance);
      require(mesh.FaceCount()<=faces,"Quad conversion increased face count");
      const int pairs=faces-mesh.FaceCount();
      require(changed==(pairs>0) && triangles-mesh.TriangleCount()==2*pairs && mesh.QuadCount()-quads==pairs,"Quad conversion culled unexpected faces");
      result->changed=static_cast<size_t>(pairs);
    }
    else if(operation==14){
      const int faces=mesh.FaceCount(),quads=mesh.QuadCount(),triangles=mesh.TriangleCount();
      require(mesh.ConvertQuadsToTriangles(),"Native quad triangulation failed");
      result->changed=static_cast<size_t>(quads-mesh.QuadCount());
      const int added=mesh.FaceCount()-faces;
      require(added>=0 && static_cast<size_t>(added)<=result->changed && mesh.TriangleCount()-triangles==quads+added,"Inconsistent quad triangulation counts");
    }
    else if(operation>=8 && operation<=13){
      require(std::isfinite(distance)&&std::isfinite(angle)&&distance>=-1.&&angle>=-1.,"Invalid nonplanar quad parameters");
      const int faces=mesh.FaceCount(),triangles=mesh.TriangleCount(),quads=mesh.QuadCount();
      // The deleting overload reads the face-to-ngon map before splitting.
      if(mesh.HasNgons())require(mesh.CreateNgonMap()!=nullptr,"Cannot map nonplanar quad n-gons");
      result->changed=mesh.ConvertNonPlanarQuadsToTriangles(distance<0.?ON_UNSET_VALUE:distance,angle<0.?ON_UNSET_VALUE:angle,operation-7,true);
      const int added=mesh.FaceCount()-faces;
      require(added>=0 && static_cast<size_t>(added)<=result->changed && static_cast<size_t>(quads-mesh.QuadCount())==result->changed && static_cast<size_t>(mesh.TriangleCount()-triangles)==result->changed+static_cast<size_t>(added),"Inconsistent nonplanar quad conversion counts");
    }
    else if(operation==5){
      require(selected && selected_count && selected_count<=vertex_limit,"Invalid selected mesh vertices");
      ON_SimpleArray<ON_COMPONENT_INDEX> components;
      for(size_t i=0;i<selected_count;++i){
        require(selected[i]<mesh.VertexUnsignedCount(),"Selected mesh vertex index is out of range");
        components.Append(ON_COMPONENT_INDEX(ON_COMPONENT_INDEX::mesh_vertex,static_cast<int>(selected[i])));
      }
      mesh.DissolveOrDelete(components);
      // Nonmanifold face sets can have no valid outer loop. Discard only the
      // invalid overlay; the surviving native facets and vertices remain intact.
      ON_SimpleArray<unsigned> workspace;
      for(unsigned i=0;i<mesh.NgonUnsignedCount();++i){
        const auto *ngon=mesh.Ngon(i);
        if(ngon && !ON_MeshNgon::IsValid(ngon,i,nullptr,mesh.VertexUnsignedCount(),mesh.FaceUnsignedCount(),mesh.m_F.Array(),workspace))mesh.RemoveNgon(i);
      }
      mesh.RemoveEmptyNgons();
      if(mesh.FaceCount() && !mesh.IsValid()){
        ON_wString details;ON_TextLog log(details);mesh.IsValid(&log);
        ON_String message(details);throw std::runtime_error(std::string("Invalid dissolved mesh: ")+message.Array());
      }
      require(mesh.FaceCount()?mesh.IsValid():mesh.VertexCount()==0,"Invalid dissolved mesh");
    }
    else throw std::runtime_error("Unknown mesh operation");
    export_mesh(mesh,*result);*out=result.release();return 1;
  }catch(const std::exception &e){if(error && capacity){std::strncpy(error,e.what(),capacity-1);error[capacity-1]=0;}return 0;}
  catch(...){if(error && capacity){std::strncpy(error,"Unknown native mesh error",capacity-1);error[capacity-1]=0;}return 0;}
}
extern "C" int vibo_mesh_edit(const ViboMeshView *input,uint32_t operation,double distance,double angle,ViboMeshResult **out,char *error,size_t capacity){
  return edit_mesh(input,operation,distance,angle,out,error,capacity);
}
extern "C" int vibo_mesh_merge_selected(const ViboMeshView *input,const size_t *selected,size_t count,int region,double distance,double angle,ViboMeshResult **out,char *error,size_t capacity){
  if(!selected || (region!=0 && region!=1) || count!=(region?1U:2U)){if(out)*out=nullptr;return 0;}
  return edit_mesh(input,1,distance,angle,out,error,capacity,selected,count,region!=0);
}
extern "C" int vibo_mesh_dissolve_vertices(const ViboMeshView *input,const size_t *selected,size_t count,ViboMeshResult **out,char *error,size_t capacity){
  return edit_mesh(input,5,0.,0.,out,error,capacity,selected,count);
}
extern "C" int vibo_mesh_result_view(const ViboMeshResult *r,ViboMeshView *v,size_t *changed){
  if(!r || !v || !changed)return 0;
  *v={r->vertices.size()/3,r->faces.size()/4,r->colors.size()/4,r->ngons.size(),r->ngon_vertices.size(),r->ngon_faces.size(),r->vertices.data(),r->faces.data(),r->colors.data(),r->ngons.data(),r->ngon_vertices.data(),r->ngon_faces.data(),r->texture_coordinates.size()/2,r->vertex_normals.size()/3,r->texture_coordinates.data(),r->vertex_normals.data()};*changed=r->changed;return 1;
}
extern "C" int vibo_mesh_cleanup_counts(const ViboMeshResult *r,size_t *removed,size_t *repaired,size_t *vertices){
  if(!r || !removed || !repaired || !vertices)return 0;
  *removed=r->removed_faces;*repaired=r->repaired_faces;*vertices=r->removed_vertices;return 1;
}
extern "C" void vibo_mesh_result_free(ViboMeshResult *r){delete r;}
extern "C" int vibo_quad_planarity(const double *points,double *distance,double *angle,char *error,size_t capacity){
  try{
    vibo_begin_open_nurbs();
    require(points && distance && angle,"Missing quad planarity input");
    for(size_t i=0;i<12;++i)require(std::isfinite(points[i]),"Nonfinite quad coordinate");
    const auto vertices=ON_3dPointListRef::FromDoubleArray(4,3,points);
    ON_MeshFace face;for(int i=0;i<4;++i)face.vi[i]=i;
    ON_PlaneEquation plane;
    require(face.GetPlaneEquation(vertices,plane),"Degenerate quad plane");
    double low=0.,high=0.;
    for(int i=1;i<4;++i){const double value=plane.ValueAt(vertices[i]);require(std::isfinite(value),"Invalid quad plane distance");low=std::min(low,value);high=std::max(high,value);}
    ON_3dVector normals[4];face.GetCornerNormals(vertices,normals);
    double cosine=1.;unsigned pairs=0;
    for(int i=0;i<2;++i)if(normals[i].IsValid() && normals[i+2].IsValid()){
      cosine=std::min(cosine,std::clamp(normals[i]*normals[i+2],-1.,1.));++pairs;
    }
    require(pairs>0 && std::isfinite(high-low),"Invalid quad corner normals");
    *distance=high-low;*angle=std::acos(cosine);return 1;
  }catch(const std::exception &e){if(error && capacity){std::strncpy(error,e.what(),capacity-1);error[capacity-1]=0;}return 0;}
  catch(...){if(error && capacity){std::strncpy(error,"Unknown quad planarity error",capacity-1);error[capacity-1]=0;}return 0;}
}
