#include "subd.h"
#include "subd_codec.h"
#include "viboceros_opennurbs.h"
#include <algorithm>
#include <array>
#include <cmath>
#include <map>
#include <set>
#include <unordered_map>

#include "subd_append.h"
#include "subd_interpolation.h"

struct ViboSubDResult {
  std::vector<uint8_t> archive, vertex_tags, edge_tags;
  std::vector<double> points, surface_points, corner_normals;
  std::vector<uint32_t> edges, face_sizes, face_vertices;
};
namespace {
using vibo_subd::require;
void point(std::vector<double>& out, const ON_3dPoint& p) {
  require(p.IsValid(), "Nonfinite SubD point");
  out.insert(out.end(),{p.x,p.y,p.z});
}
std::unique_ptr<ViboSubDResult> extract(const ON_SubD& subd, bool evaluated=false, bool normals=false) {
  auto out=std::make_unique<ViboSubDResult>();
  if (!evaluated) out->archive=vibo_subd::encode(subd);
  std::unordered_map<const ON_SubDVertex*,uint32_t> indices;
  auto vit=subd.VertexIterator();
  for (auto* v=vit.FirstVertex(); v; v=vit.NextVertex()) {
    indices.emplace(v,static_cast<uint32_t>(indices.size()));
    const auto surface=v->SurfacePoint();
    point(out->points,evaluated ? surface : v->ControlNetPoint());
    point(out->surface_points,surface);
    out->vertex_tags.push_back(static_cast<uint8_t>(v->m_vertex_tag));
  }
  auto eit=subd.EdgeIterator();
  for (auto* e=eit.FirstEdge(); e; e=eit.NextEdge()) {
    out->edges.push_back(indices.at(e->m_vertex[0]));
    out->edges.push_back(indices.at(e->m_vertex[1]));
    out->edge_tags.push_back(static_cast<uint8_t>(e->m_edge_tag));
  }
  auto fit=subd.FaceIterator();
  for (auto* f=fit.FirstFace(); f; f=fit.NextFace()) {
    out->face_sizes.push_back(f->EdgeCount());
    for (unsigned i=0; i<f->EdgeCount(); ++i) {
      out->face_vertices.push_back(indices.at(f->Vertex(i)));
      if(normals) {
        auto normal=f->Vertex(i)->SurfaceNormal(f,true);
        // Undefined normals in degenerate sectors remain explicit zero vectors.
        if(!normal.IsValid()) normal=ON_3dVector::ZeroVector;
        out->corner_normals.insert(out->corner_normals.end(),{normal.x,normal.y,normal.z});
      }
    }
  }
  return out;
}
template<class Result,class F> int guard(Result** out, char* error, size_t capacity, F&& f) {
  if(out) *out=nullptr;
  if(error && capacity) error[0]=0;
  try {
    require(out,"Missing SubD output"); vibo_begin_open_nurbs();
    *out=f().release(); return 1;
  } catch(const std::exception& e) {
    if(error && capacity) { const size_t n=std::min(capacity-1,std::strlen(e.what())); std::memcpy(error,e.what(),n); error[n]=0; }
  } catch(...) {
    if(error && capacity) { const char* message="Unknown SubD failure"; const size_t n=std::min(capacity-1,std::strlen(message)); std::memcpy(error,message,n);error[n]=0; }
  }
  return 0;
}
}
extern "C" int vibo_subd_create(const double* points,size_t point_count,
    const uint32_t* sizes,size_t face_count,const uint32_t* corners,size_t corner_count,
    ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    require(points && sizes && corners && point_count>=3 && point_count<=vibo_subd::component_limit &&
      face_count>0 && face_count<=vibo_subd::component_limit && corner_count<=4*vibo_subd::component_limit,
      "Invalid or excessive SubD control net");
    ON_SubD subd;
    std::vector<ON_SubDVertex*> vertices;
    for(size_t i=0;i<point_count;++i) {
      ON_3dPoint p(points+3*i);require(p.IsValid(),"Invalid SubD control point");
      auto* v=subd.AddVertex(ON_SubDVertexTag::Unset,points+3*i);
      require(v,"Cannot allocate SubD vertex");vertices.push_back(v);
    }
    struct Edge { ON_SubDEdge* edge; uint32_t start; unsigned count; };
    std::map<std::pair<uint32_t,uint32_t>,Edge> edges;
    size_t offset=0;
    for(size_t i=0;i<face_count;++i) {
      const size_t n=sizes[i];
      require(n>=3 && n<=ON_SubDFace::MaximumEdgeCount && offset<=corner_count && n<=corner_count-offset,"Invalid SubD face size");
      std::set<uint32_t> distinct;
      ON_SimpleArray<ON_SubDEdgePtr> boundary;
      for(size_t j=0;j<n;++j) {
        const auto a=corners[offset+j],b=corners[offset+(j+1)%n];
        require(a<point_count && b<point_count && distinct.insert(a).second,"Invalid or repeated SubD face vertex");
        const auto key=std::minmax(a,b);
        auto found=edges.find(key);
        if(found==edges.end()) {
          auto* e=subd.AddEdge(vertices[a],vertices[b]);require(e,"Cannot allocate SubD edge");
          found=edges.emplace(key,Edge{e,a,0}).first;
        } else require(found->second.count==1 && found->second.start!=a,"Nonmanifold or inconsistently oriented SubD edge");
        ++found->second.count;
        boundary.Append(ON_SubDEdgePtr::Create(found->second.edge,found->second.start==a ? 0 : 1));
      }
      require(subd.AddFace(boundary),"Cannot create SubD face");offset+=n;
    }
    require(offset==corner_count,"Trailing SubD face corners");
    subd.UpdateAllTagsAndSectorCoefficients(true);
    return extract(subd);
  });
}
extern "C" int vibo_subd_append(const uint8_t* bytes,size_t count,const double* points,size_t point_count,
    const uint32_t* sizes,size_t face_count,const uint32_t* corners,size_t corner_count,
    ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    auto subd=vibo_subd::decode(bytes,count);
    vibo_subd::append_polygons(*subd,points,point_count,sizes,face_count,corners,corner_count);
    return extract(*subd);
  });
}
extern "C" int vibo_subd_set_tags(const uint8_t* bytes,size_t count,int vertex,
    const size_t* indices,const uint8_t* tags,size_t tag_count,ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    require((vertex==0 || vertex==1) && tag_count<=vibo_subd::component_limit &&
      (tag_count==0 || (indices && tags)),"Invalid SubD tag input");
    auto subd=vibo_subd::decode(bytes,count);
    std::vector<ON_SubDComponentPtr> components;
    if(vertex) {auto it=subd->VertexIterator();for(auto* v=it.FirstVertex();v;v=it.NextVertex()) components.push_back(ON_SubDComponentPtr::Create(v));}
    else {auto it=subd->EdgeIterator();for(auto* e=it.FirstEdge();e; e=it.NextEdge()) components.push_back(ON_SubDComponentPtr::Create(e));}
    for(size_t i=0;i<tag_count;++i) {
      require(indices[i]<components.size(),"SubD tag index outside component table");
      require(tags[i]>=1 && tags[i]<=(vertex?4:2),"Unsupported SubD tag");
      const auto component=components[indices[i]];
      if(vertex) subd->SetVertexTags(&component,1,static_cast<ON_SubDVertexTag>(tags[i]));
      else subd->SetEdgeTags(&component,1,static_cast<ON_SubDEdgeTag>(tags[i]));
    }
    subd->UpdateAllTagsAndSectorCoefficients(false);subd->ClearEvaluationCache();return extract(*subd);
  });
}
extern "C" int vibo_subd_read(const uint8_t* bytes,size_t n,ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{return extract(*vibo_subd::decode(bytes,n));});
}
extern "C" int vibo_subd_edit_points(const uint8_t* bytes,size_t n,const double* points,size_t count,
    ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    auto subd=vibo_subd::decode(bytes,n);
    require(points && count==subd->VertexCount(),"SubD edit point count mismatch");
    auto vit=subd->VertexIterator();size_t i=0;
    for(auto* v=vit.FirstVertex();v;v=vit.NextVertex(),++i) {
      const ON_3dPoint p(points+3*i);require(p.IsValid(),"Invalid edited SubD point");
      require(const_cast<ON_SubDVertex*>(v)->SetControlNetPoint(p,false),"Cannot edit SubD vertex");
    }
    subd->ClearEvaluationCache();return extract(*subd);
  });
}
extern "C" int vibo_subd_transform(const uint8_t* bytes,size_t n,const double* matrix,
    ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    require(matrix,"Missing SubD transformation");
    ON_Xform transform;
    for(unsigned i=0;i<4;++i) for(unsigned j=0;j<4;++j) {
      require(std::isfinite(matrix[4*i+j]),"Nonfinite SubD transformation");
      transform[i][j]=matrix[4*i+j];
    }
    require(transform[3][0]==0 && transform[3][1]==0 && transform[3][2]==0 && transform[3][3]==1,
      "SubD transformation must be affine");
    auto subd=vibo_subd::decode(bytes,n);
    require(subd->Transform(transform),"SubD transformation failed");
    return extract(*subd);
  });
}
extern "C" int vibo_subd_control_mesh(const uint8_t* bytes,size_t count,ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    auto subd=vibo_subd::decode(bytes,count);ON_Mesh mesh;
    require(subd->GetControlNetMesh(&mesh,ON_SubDGetControlNetMeshPriority::Geometry),"Cannot get SubD control mesh");
    auto result=std::make_unique<ViboSubDResult>();
    for(int i=0;i<mesh.VertexCount();++i) {
      point(result->points,mesh.Vertex(i));point(result->surface_points,mesh.Vertex(i));result->vertex_tags.push_back(0);
    }
    for(int i=0;i<mesh.FaceCount();++i) {
      const auto& face=mesh.m_F[i];const unsigned n=face.IsTriangle()?3:4;result->face_sizes.push_back(n);
      for(unsigned j=0;j<n;++j) result->face_vertices.push_back(static_cast<uint32_t>(face.vi[j]));
    }
    return result;
  });
}
extern "C" int vibo_subd_tessellate(const uint8_t* bytes,size_t n,unsigned density,int normals,
    ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    require(normals==0 || normals==1,"Invalid SubD normal request");
    require(density>=1 && density<=6,"SubD display density must be between 1 and 6");
    auto subd=vibo_subd::decode(bytes,n);
    // Count in 64 bits before subdivision; GlobalSubdivideQuadCount() returns u32.
    uint64_t quads=0;auto fit=subd->FaceIterator();
    for(auto* f=fit.FirstFace();f;f=fit.NextFace()) quads+=f->EdgeCount();
    for(unsigned i=1;i<density;++i) quads*=4;
    require(quads<=1000000,"SubD tessellation face budget exceeded");
    require(subd->GlobalSubdivide(density),"SubD subdivision failed");
    return extract(*subd,true,normals!=0);
  });
}
extern "C" int vibo_subd_view(const ViboSubDResult* r,ViboSubDView* v) {
  if(!r || !v) return 0;
  *v={r->archive.size(),r->points.size()/3,r->edges.size()/2,r->face_sizes.size(),r->face_vertices.size(),r->corner_normals.size()/3,
    r->archive.data(),r->points.data(),r->surface_points.data(),r->vertex_tags.data(),r->edges.data(),r->edge_tags.data(),r->face_sizes.data(),r->face_vertices.data(),r->corner_normals.data()};return 1;
}
extern "C" void vibo_subd_free(ViboSubDResult* r) { delete r; }

extern "C" int vibo_subd_weights(const uint8_t* bytes,size_t n,ViboSubDWeights** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{auto subd=vibo_subd::decode(bytes,n);return vibo_subd::interpolation_weights(*subd);});
}
extern "C" int vibo_subd_weights_view(const ViboSubDWeights* w,ViboSubDWeightsView* view) {
  if(!w || !view) return 0;
  *view={w->offsets.size()-1,w->values.size(),w->offsets.data(),w->columns.data(),w->values.data()};return 1;
}
extern "C" void vibo_subd_weights_free(ViboSubDWeights* w) {delete w;}
