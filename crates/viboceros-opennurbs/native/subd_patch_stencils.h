#ifndef VIBOCEROS_SUBD_PATCH_STENCILS_H
#define VIBOCEROS_SUBD_PATCH_STENCILS_H
namespace vibo_subd {
inline const ON_SubDFace* stencil_neighbor(const ON_SubDEdge* edge,const ON_SubDFace* source) {
  const ON_SubDFace* neighbor=nullptr;
  for(unsigned i=0;i<edge->FaceCount();++i) if(edge->Face(i)!=source) {
    require(!neighbor,"Ambiguous SubD stencil neighbor");neighbor=edge->Face(i);
  }
  require(neighbor && neighbor->EdgeCount()==4,"Missing quad stencil neighbor");return neighbor;
}
inline const ON_SubDVertex* stencil_outer_vertex(const ON_SubDFace* face,
    const ON_SubDVertex* a,const ON_SubDVertex* b) {
  for(unsigned i=0;i<4;++i) if(face->Vertex(i)==a) {
    const auto* previous=face->Vertex((i+3)%4);
    const auto* next=face->Vertex((i+1)%4);
    require(previous==b || next==b,"Missing stencil neighbor side");
    return previous==b?next:previous;
  }
  throw std::runtime_error("Missing stencil neighbor vertex");
}
// Layout: refined control grid, four clockwise halo rows, four limit corners.
// The outer diagonal controls are constructed independently in Rust.
inline void finish_patch_stencils(ViboSubDFaceGrids& out,
    const std::vector<FaceGridTrack>& tracks,const std::vector<const ON_SubDVertex*>& vertices,
    bool allow_partial=false) {
  auto grids=out.grids;size_t count=0;
  for(auto& grid:grids) {grid.offset=count;count+=(grid.side+3)*(grid.side+3);}
  require(count<=4000000,"SubD patch stencil budget exceeded");
  out.points.assign(3*count,ON_DBL_QNAN);
  auto assign=[&](size_t index,const ON_3dPoint& p) {
    require(p.IsValid(),"Nonfinite SubD stencil control");
    auto* slot=out.points.data()+3*index;
    require(!std::isfinite(slot[0]) || (slot[0]==p.x && slot[1]==p.y && slot[2]==p.z),
      "Inconsistent SubD stencil side");
    slot[0]=p.x;slot[1]=p.y;slot[2]=p.z;
  };
  for(size_t i=0;i<grids.size();++i) {
    const auto& old=out.grids[i];const auto& grid=grids[i];const size_t s=grid.side,stride=s+1;
    for(size_t j=0;j<stride*stride;++j) {
      const auto* vertex=vertices[old.offset+j];require(vertex,"Incomplete SubD stencil grid");
      assign(grid.offset+j,vertex->ControlNetPoint());
    }
    const std::array<size_t,4> corners={0,s,s*stride+s,s*stride};
    for(unsigned j=0;j<4;++j) assign(grid.offset+stride*stride+4*stride+j,
      vertices[old.offset+corners[j]]->SurfacePoint());
  }
  for(const auto& t:tracks) {
    auto& grid=grids[t.root];const size_t s=grid.side,stride=s+1;
    const std::array<bool,4> boundary={t.y==0,t.x+1==s,t.y+1==s,t.x==0};
    const std::array<size_t,4> station={t.x,t.y,s-t.x-1,s-t.y-1};
    for(unsigned side=0;side<4;++side) if(boundary[side]) {
      const auto* a=t.vertices[side];const auto* b=t.vertices[(side+1)%4];
      const auto* edge=grid_edge(t.face,a,b);
      if(edge->IsSharp()) {
        require(allow_partial,"Unresolved soft-sharp SubD stencil edge");
        if(station[side]==0)grid.crease_corners|=size_t(1)<<(4+side);
        if(station[side]+1==s)grid.crease_corners|=size_t(1)<<(4+(side+1)%4);
      }
      std::array<ON_3dPoint,2> halo;
      if(edge->IsCrease() || edge->FaceCount()==1) {
        const auto A=a->ControlNetPoint(),B=b->ControlNetPoint();
        const auto P=t.vertices[(side+3)%4]->ControlNetPoint();
        const auto Q=t.vertices[(side+2)%4]->ControlNetPoint();
        halo={A+(A-P),B+(B-Q)};
      } else {
        const auto* neighbor=stencil_neighbor(edge,t.face);
        halo={stencil_outer_vertex(neighbor,a,b)->ControlNetPoint(),
          stencil_outer_vertex(neighbor,b,a)->ControlNetPoint()};
      }
      const size_t base=grid.offset+stride*stride+side*stride+station[side];
      assign(base,halo[0]);assign(base+1,halo[1]);
    }
  }
  require(std::all_of(out.points.begin(),out.points.end(),[](double v){return std::isfinite(v);}),
    "Incomplete SubD patch stencil");
  out.grids=std::move(grids);
}
}
#endif
