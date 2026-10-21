#ifndef VIBOCEROS_SUBD_SUBDIVISION_H
#define VIBOCEROS_SUBD_SUBDIVISION_H
// Preflight persistent topology before the public OpenNURBS subdivision call.
#include <unordered_set>
namespace vibo_subd {
inline void subdivision_budget(uint64_t v, uint64_t e, uint64_t f, uint64_t c) {
  require(v<=component_limit && f<=component_limit && e<=4*component_limit && c<=4*component_limit,
    "SubD subdivision component budget exceeded");
}
inline void subdivide(ON_SubD& subd, unsigned count, const size_t* indices, size_t n, bool local) {
  require(count>=1 && count<=8 && (!local || count==1),"Invalid SubD subdivision count");
  uint64_t v=subd.VertexCount(),e=subd.EdgeCount(),f=subd.FaceCount(),c=0;
  std::vector<const ON_SubDFace*> faces;
  auto it=subd.FaceIterator();
  for(auto* face=it.FirstFace();face;face=it.NextFace()) {faces.push_back(face);c+=face->EdgeCount();}
  subdivision_budget(v,e,f,c);
  if(!local) {
    for(unsigned i=0;i<count;++i) {
      const uint64_t nv=v+e+f,ne=2*e+c,nf=c,nc=4*c;
      subdivision_budget(nv,ne,nf,nc);v=nv;e=ne;f=nf;c=nc;
    }
    require(subd.GlobalSubdivide(count),"SubD global subdivision failed");
    return;
  }
  require(indices && n>0 && n<=component_limit,"Invalid SubD face selection");
  std::unordered_set<const ON_SubDFace*> selected;
  std::unordered_set<const ON_SubDEdge*> touched;
  uint64_t corners=0;
  for(size_t i=0;i<n;++i) {
    require(indices[i]<faces.size(),"SubD subdivision face index out of range");
    auto* face=faces[indices[i]];
    if(!selected.insert(face).second) continue;
    corners+=face->EdgeCount();
    for(unsigned j=0;j<face->EdgeCount();++j) touched.insert(face->Edge(j));
  }
  uint64_t extra=0;
  for(auto* face:faces) if(selected.count(face)==0) {
    unsigned split=0;
    for(unsigned j=0;j<face->EdgeCount();++j) if(touched.count(face->Edge(j))) ++split;
    require(face->EdgeCount()+split<=ON_SubDFace::MaximumEdgeCount,"SubD neighbor face edge limit exceeded");
    extra+=split;
  }
  subdivision_budget(v+touched.size()+selected.size(),e+touched.size()+corners,
    f-selected.size()+corners,c+3*corners+extra);
  if(selected.size()==faces.size()) {
    require(subd.GlobalSubdivide(1),"SubD global subdivision failed");
    return;
  }
  std::vector<const ON_SubDFace*> ordered;
  for(auto* face:faces) if(selected.count(face)) ordered.push_back(face);
  require(subd.LocalSubdivide(ordered.data(),ordered.size()),"SubD local subdivision failed");
}
}
#endif
