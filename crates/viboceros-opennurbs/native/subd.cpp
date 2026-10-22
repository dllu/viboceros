#include "subd.h"
#include "subd_codec.h"
#include "mesh_codec.h"
#include "viboceros_opennurbs.h"
#include <algorithm>
#include <array>
#include <cmath>
#include <map>
#include <set>
#include <unordered_map>

#include "subd_append.h"
#include "subd_interpolation.h"
#include "subd_subdivision.h"

struct ViboSubDResult {
  std::vector<uint8_t> archive, vertex_tags, edge_tags;
  std::vector<double> points, surface_points, corner_normals;
  std::vector<uint32_t> edges, face_sizes, face_vertices, face_sources, selected_faces;
  size_t boundary_edge_count=0;
};
namespace {
using vibo_subd::require;
void point(std::vector<double>& out, const ON_3dPoint& p) {
  require(p.IsValid(), "Nonfinite SubD point");
  out.insert(out.end(),{p.x,p.y,p.z});
}
std::unique_ptr<ViboSubDResult> extract(const ON_SubD& subd, bool evaluated=false, bool normals=false, const std::unordered_map<unsigned,uint32_t>* sources=nullptr, const std::unordered_set<unsigned>* retained_faces=nullptr) {
  auto out=std::make_unique<ViboSubDResult>();
  if (!evaluated) out->archive=vibo_subd::encode(subd);
  std::unordered_map<const ON_SubDVertex*,uint32_t> indices;
  std::vector<const ON_SubDVertex*> vertices;
  auto vit=subd.VertexIterator();
  for(auto* v=vit.FirstVertex();v;v=vit.NextVertex()) vertices.push_back(v);
  if(!evaluated) std::sort(vertices.begin(),vertices.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  for (auto* v:vertices) {
    indices.emplace(v,static_cast<uint32_t>(indices.size()));
    const auto surface=v->SurfacePoint();
    point(out->points,evaluated ? surface : v->ControlNetPoint());
    point(out->surface_points,surface);
    out->vertex_tags.push_back(static_cast<uint8_t>(v->m_vertex_tag));
  }
  std::vector<const ON_SubDEdge*> edges;
  auto eit=subd.EdgeIterator();
  for(auto* e=eit.FirstEdge();e;e=eit.NextEdge()) edges.push_back(e);
  if(!evaluated) std::sort(edges.begin(),edges.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  for (auto* e:edges) {
    out->edges.push_back(indices.at(e->m_vertex[0]));
    out->edges.push_back(indices.at(e->m_vertex[1]));
    out->edge_tags.push_back(static_cast<uint8_t>(e->m_edge_tag));
    if(e->FaceCount()==1) ++out->boundary_edge_count;
  }
  std::vector<const ON_SubDFace*> faces;
  auto fit=subd.FaceIterator();
  for(auto* f=fit.FirstFace();f;f=fit.NextFace()) faces.push_back(f);
  if(!evaluated) std::sort(faces.begin(),faces.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  for (auto* f:faces) {
    if(retained_faces && retained_faces->count(f->m_id)==0) out->selected_faces.push_back(static_cast<uint32_t>(out->face_sizes.size()));
    out->face_sizes.push_back(f->EdgeCount());
    if(sources) out->face_sources.push_back(sources->at(f->m_level_zero_face_id));
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
#include "subd_crease.h"
extern "C" int vibo_subd_crease(const uint8_t* bytes,size_t count,
    const size_t* edges,size_t edge_count,const size_t* vertices,size_t vertex_count,
    int crease,size_t* changed,ViboSubDResult** out,char* error,size_t capacity) {
  if(changed) *changed=0;
  return guard(out,error,capacity,[&]{
    require(changed && (crease==0 || crease==1),"Invalid crease operation");
    auto subd=vibo_subd::decode(bytes,count);
    *changed=vibo_subd::crease_components(*subd,edges,edge_count,vertices,vertex_count,crease!=0);
    return extract(*subd);
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
    std::unordered_map<unsigned,uint32_t> sources;
    auto source_it=subd->FaceIterator();
    for(auto* f=source_it.FirstFace();f;f=source_it.NextFace()) {
      sources.emplace(f->m_id,static_cast<uint32_t>(sources.size()));
      // This is a private display copy. Reset lineage to its current active net,
      // including archives whose active level is already globally subdivided.
      const_cast<ON_SubDFace*>(f)->m_level_zero_face_id=f->m_id;
    }
    require(subd->GlobalSubdivide(density),"SubD subdivision failed");
    return extract(*subd,true,normals!=0,&sources);
  });
}
extern "C" int vibo_subd_view(const ViboSubDResult* r,ViboSubDView* v) {
  if(!r || !v) return 0;
  *v={r->archive.size(),r->points.size()/3,r->edges.size()/2,r->face_sizes.size(),r->face_vertices.size(),r->corner_normals.size()/3,
    r->archive.data(),r->points.data(),r->surface_points.data(),r->vertex_tags.data(),r->edges.data(),r->edge_tags.data(),r->face_sizes.data(),r->face_vertices.data(),r->corner_normals.data(),r->boundary_edge_count,r->face_sources.size(),r->face_sources.data(),r->selected_faces.size(),r->selected_faces.data()};return 1;
}
extern "C" void vibo_subd_free(ViboSubDResult* r) { delete r; }

extern "C" int vibo_subd_cylinder(const double* frame,double radius,double height,const uint32_t* counts,uint32_t cap,ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    require(frame && counts && counts[0]>=3 && counts[1]>=1 && counts[0]<=vibo_subd::component_limit && counts[1]<=vibo_subd::component_limit,
      "Invalid SubD cylinder counts");
    require(cap>=1 && cap<=3,"Invalid SubD cylinder cap style");
    const uint64_t n=counts[0],h=counts[1];
    const bool triangles=cap==2 || (cap==3 && n>4 && n%2!=0);
    const bool center=(triangles && n>3) || (cap==3 && n>4);
    const uint64_t cap_faces=cap==1?0:triangles?(n<=3?2:2*n):n<=4?2:n;
    require(n*(h+1)+(center?2:0)<=vibo_subd::component_limit && n*h+cap_faces<=vibo_subd::component_limit,
      "SubD cylinder component budget exceeded");
    const ON_Plane plane(ON_3dPoint(frame),ON_3dVector(frame+3),ON_3dVector(frame+6));
    require(plane.IsValid() && std::isfinite(radius) && radius>0. && std::isfinite(height) && height>0.,"Invalid SubD cylinder dimensions");
    ON_SubD subd;
    const ON_Cylinder cylinder(ON_Circle(plane,radius),height);
    require(ON_SubD::CreateCylinder(cylinder,counts[0],counts[1],ON_SubDEndCapStyleFromUnsigned(cap),ON_SubDEdgeTag::Smooth,ON_SubDComponentLocation::Surface,&subd)==&subd,
      "Native SubD cylinder construction failed");
    return extract(subd);
  });
}

extern "C" int vibo_subd_box(const double* corners,const uint32_t* counts,ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    require(corners && counts,"Missing SubD box parameters");
    for(unsigned i=0;i<3;++i)
      require(counts[i]>0 && counts[i]<=vibo_subd::component_limit,"Invalid SubD box face count");
    const uint64_t x=counts[0],y=counts[1],z=counts[2];
    require(2*(x*y+x*z+y*z)+2<=vibo_subd::component_limit,"SubD box component budget exceeded");
    ON_3dPoint points[8];
    for(unsigned i=0;i<8;++i) {points[i]=ON_3dPoint(corners+3*i);require(points[i].IsValid(),"Invalid SubD box corner");}
    ON_SubD subd;
    require(ON_SubD::CreateSubDBox(points,ON_SubDEdgeTag::Smooth,counts[0],counts[1],counts[2],&subd)==&subd,
      "Native SubD box construction failed");
    return extract(subd);
  });
}

extern "C" int vibo_subd_weights(const uint8_t* bytes,size_t n,ViboSubDWeights** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{auto subd=vibo_subd::decode(bytes,n);return vibo_subd::interpolation_weights(*subd);});
}
extern "C" int vibo_subd_weights_view(const ViboSubDWeights* w,ViboSubDWeightsView* view) {
  if(!w || !view) return 0;
  *view={w->offsets.size()-1,w->values.size(),w->offsets.data(),w->columns.data(),w->values.data()};return 1;
}
extern "C" void vibo_subd_weights_free(ViboSubDWeights* w) {delete w;}

extern "C" int vibo_subd_subdivide(const uint8_t* bytes,size_t n,unsigned count,
    const size_t* faces,size_t face_count,int local,ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    require(local==0 || local==1,"Invalid SubD subdivision mode");
    auto subd=vibo_subd::decode(bytes,n);
    std::unordered_set<unsigned> retained;
    if(local) {
      std::vector<unsigned> ids;auto fit=subd->FaceIterator();
      for(auto* f=fit.FirstFace();f;f=fit.NextFace()) ids.push_back(f->m_id);
      std::unordered_set<size_t> selected;
      require(faces && face_count>0 && face_count<=vibo_subd::component_limit,"Invalid SubD face selection");
      for(size_t i=0;i<face_count;++i) {require(faces[i]<ids.size(),"SubD subdivision face index out of range");selected.insert(faces[i]);}
      for(size_t i=0;i<ids.size();++i) if(selected.count(i)==0) retained.insert(ids[i]);
    }
    vibo_subd::subdivide(*subd,count,faces,face_count,local!=0);
    // Persist the refined net as level zero. Otherwise Write/Read can restore
    // an older level, and subsequent local edits can mix level indices.
    subd->ClearInactiveLevels();
    return extract(*subd,false,false,nullptr,local?&retained:nullptr);
  });
}

extern "C" int vibo_subd_from_mesh(const ViboMeshView* input,int corners,int creases,
    ViboSubDResult** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    require(input && (corners==0 || corners==1) && (creases==0 || creases==1),"Invalid mesh-to-SubD options");
    ON_Mesh mesh;vibo_mesh::import_mesh(*input,mesh);
    ON_SubDFromMeshParameters parameters=ON_SubDFromMeshParameters::Smooth;
    parameters.SetInteriorCreaseOption(creases?ON_SubDFromMeshParameters::InteriorCreaseOption::AtMeshDoubleEdge:ON_SubDFromMeshParameters::InteriorCreaseOption::None);
    parameters.SetConvexCornerOption(corners?ON_SubDFromMeshParameters::ConvexCornerOption::AtMeshCorner:ON_SubDFromMeshParameters::ConvexCornerOption::None);
    std::unique_ptr<ON_SubD> subd(ON_SubD::CreateFromMesh(&mesh,&parameters,nullptr));
    require(subd!=nullptr,"Cannot convert mesh to SubD");
    vibo_subd::check(*subd);
    uint64_t corner_count=0;auto fit=subd->FaceIterator();
    for(auto* f=fit.FirstFace();f;f=fit.NextFace()) corner_count+=f->EdgeCount();
    require(corner_count<=4*vibo_subd::component_limit,"Converted SubD corner budget exceeded");
    return extract(*subd);
  });
}
