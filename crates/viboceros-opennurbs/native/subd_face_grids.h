#ifndef VIBOCEROS_SUBD_FACE_GRIDS_H
#define VIBOCEROS_SUBD_FACE_GRIDS_H
// Limit points retain the source face chart through every subdivision step.
// N-gons have one chart per corner, starting after their first subdivision.
struct ViboSubDFaceGrids {
  std::vector<ViboSubDFaceGridInfo> grids;
  std::vector<double> points;
};
namespace vibo_subd {
struct FaceGridTrack {
  const ON_SubDFace* face;
  std::array<const ON_SubDVertex*,4> vertices;
  size_t root, x, y, side;
};
inline const ON_SubDFace* grid_child_face(const ON_SubDVertex* edge_point,
    const ON_SubDVertex* corner, const ON_SubDVertex* center) {
  require(edge_point && corner && center,"Missing subdivided face vertex");
  const ON_SubDFace* found=nullptr;
  // An edge point's fan is bounded for manifold inputs. Avoid repeatedly
  // searching the unbounded fans at original vertices or n-gon centers.
  for(unsigned i=0;i<edge_point->FaceCount();++i) {
    const auto* face=edge_point->Face(i);
    bool has_corner=false,has_center=false;
    for(unsigned j=0;j<face->EdgeCount();++j) {
      has_corner|=face->Vertex(j)==corner;
      has_center|=face->Vertex(j)==center;
    }
    if(has_corner && has_center) {
      require(!found && face->EdgeCount()==4,"Ambiguous subdivided face lineage");
      found=face;
    }
  }
  require(found,"Missing subdivided face lineage");return found;
}
inline const ON_SubDEdge* grid_edge(const ON_SubDFace* face,
    const ON_SubDVertex* a,const ON_SubDVertex* b) {
  for(unsigned i=0;i<face->EdgeCount();++i) {
    const auto* edge=face->Edge(i);
    if((edge->m_vertex[0]==a && edge->m_vertex[1]==b) ||
       (edge->m_vertex[0]==b && edge->m_vertex[1]==a)) return edge;
  }
  throw std::runtime_error("Missing oriented face edge");
}
inline std::unique_ptr<ViboSubDFaceGrids> face_grids(ON_SubD& subd,unsigned density) {
  require(density>=1 && density<=6,"SubD face density must be between 1 and 6");
  std::vector<const ON_SubDFace*> source;
  uint64_t v=subd.VertexCount(),e=subd.EdgeCount(),f=subd.FaceCount(),c=0;
  auto fit=subd.FaceIterator();
  for(auto* face=fit.FirstFace();face;face=fit.NextFace()) {
    require(face->EdgeCount()>=3,"Invalid source face");
    source.push_back(face);c+=face->EdgeCount();
  }
  std::sort(source.begin(),source.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  auto eit=subd.EdgeIterator();
  for(auto* edge=eit.FirstEdge();edge;edge=eit.NextEdge())
    require(edge->FaceCount()<=2,"Nonmanifold SubD face grid");
  // Check growth before asking the kernel to allocate any refined level.
  for(unsigned i=0;i<density;++i) {
    const uint64_t nv=v+e+f,ne=2*e+c,nf=c;
    require(nv<=4000000 && ne<=4000000 && nf<=1000000,
      "SubD face subdivision budget exceeded");
    v=nv;e=ne;f=nf;c=4*nf;
  }
  auto out=std::make_unique<ViboSubDFaceGrids>();
  std::vector<FaceGridTrack> tracks;
  std::vector<std::pair<const ON_SubDFace*,size_t>> polygons;
  size_t point_count=0;
  for(size_t i=0;i<source.size();++i) {
    const auto* face=source[i];const bool quad=face->EdgeCount()==4;
    const size_t side=size_t(1)<<(density-(quad?0:1));
    const size_t count=quad?1:face->EdgeCount();
    require(count*(side+1)*(side+1)<=4000000-point_count,"SubD face sample budget exceeded");
    const size_t root=out->grids.size();
    for(size_t j=0;j<count;++j) {
      out->grids.push_back({i,quad?SIZE_MAX:j,side,point_count});
      point_count+=(side+1)*(side+1);
    }
    if(quad) tracks.push_back({face,{face->Vertex(0),face->Vertex(1),
      face->Vertex(2),face->Vertex(3)},root,0,0,side});
    else polygons.push_back({face,root});
  }
  for(unsigned level=0;level<density;++level) {
    // Resolve edges while parent components still belong to the active level.
    std::vector<std::array<const ON_SubDEdge*,4>> edges;
    edges.reserve(tracks.size());
    for(const auto& t:tracks) {
      std::array<const ON_SubDEdge*,4> boundary;
      for(unsigned j=0;j<4;++j) boundary[j]=grid_edge(t.face,t.vertices[j],t.vertices[(j+1)%4]);
      edges.push_back(boundary);
    }
    require(subd.GlobalSubdivide(1),"SubD face subdivision failed");
    std::vector<FaceGridTrack> next;
    size_t polygon_count=0;
    if(level==0) for(const auto& polygon:polygons) polygon_count+=polygon.first->EdgeCount();
    next.reserve(4*tracks.size()+polygon_count);
    for(size_t i=0;i<tracks.size();++i) {
      const auto& t=tracks[i];const size_t h=t.side/2;
      require(h>0,"Invalid face grid depth");
      std::array<const ON_SubDVertex*,4> a,m;
      for(unsigned j=0;j<4;++j) {a[j]=t.vertices[j]->m_subd_point1;m[j]=edges[i][j]->m_subd_point1;}
      const auto* center=t.face->m_subd_point1;
      const std::array<const ON_SubDFace*,4> child={
        grid_child_face(m[0],a[0],center),grid_child_face(m[1],a[1],center),
        grid_child_face(m[2],a[2],center),grid_child_face(m[3],a[3],center)};
      next.push_back({child[0],{a[0],m[0],center,m[3]},t.root,t.x,t.y,h});
      next.push_back({child[1],{m[0],a[1],m[1],center},t.root,t.x+h,t.y,h});
      next.push_back({child[2],{center,m[1],a[2],m[2]},t.root,t.x+h,t.y+h,h});
      next.push_back({child[3],{m[3],center,m[2],a[3]},t.root,t.x,t.y+h,h});
    }
    if(level==0) for(const auto& polygon:polygons) {
      const auto* face=polygon.first;const unsigned n=face->EdgeCount();
      for(unsigned j=0;j<n;++j) {
        const auto* a=face->Vertex(j)->m_subd_point1;
        const auto* m=face->Edge(j)->m_subd_point1;
        const auto* previous=face->Edge((j+n-1)%n)->m_subd_point1;
        const auto* center=face->m_subd_point1;
        const size_t root=polygon.second+j;
        next.push_back({grid_child_face(m,a,center),{a,m,center,previous},
          root,0,0,out->grids[root].side});
      }
    }
    tracks=std::move(next);
  }
  std::vector<const ON_SubDVertex*> vertices(point_count,nullptr);
  for(const auto& t:tracks) {
    require(t.side==1,"Incomplete face grid subdivision");
    const auto& grid=out->grids[t.root];const size_t stride=grid.side+1;
    const size_t base=grid.offset+t.y*stride+t.x;
    const std::array<size_t,4> offsets={base,base+1,base+stride+1,base+stride};
    for(unsigned j=0;j<4;++j) {
      auto& slot=vertices[offsets[j]];
      require(!slot || slot==t.vertices[j],"Inconsistent face grid chart");slot=t.vertices[j];
    }
  }
  out->points.reserve(3*point_count);
  for(const auto* vertex:vertices) {
    require(vertex,"Incomplete face grid points");point(out->points,vertex->SurfacePoint());
  }
  return out;
}
}
extern "C" int vibo_subd_face_grids(const uint8_t* bytes,size_t count,unsigned density,
    ViboSubDFaceGrids** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    auto subd=vibo_subd::decode(bytes,count);return vibo_subd::face_grids(*subd,density);
  });
}
extern "C" int vibo_subd_face_grids_view(const ViboSubDFaceGrids* r,ViboSubDFaceGridsView* v) {
  if(!r || !v) return 0;
  *v={r->grids.size(),r->points.size()/3,r->grids.data(),r->points.data()};return 1;
}
extern "C" void vibo_subd_face_grids_free(ViboSubDFaceGrids* r) {delete r;}
#endif
