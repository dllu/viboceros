#ifndef VIBOCEROS_SUBD_EDGES_H
#define VIBOCEROS_SUBD_EDGES_H
// Read-only limit-edge samples, with exact topological lineage to source IDs.
struct ViboSubDEdgeSamples {
  std::vector<size_t> offsets;
  std::vector<double> points;
};
namespace vibo_subd {
struct EdgeTrack {
  const ON_SubDEdge* edge;
  const ON_SubDVertex* start;
  const ON_SubDVertex* end;
};
inline const ON_SubDEdge* child_edge(const ON_SubDVertex* center, const ON_SubDVertex* end) {
  require(center && end, "Missing subdivided edge vertex");
  for(unsigned i=0;i<center->EdgeCount();++i) {
    const auto* e=center->Edge(i);
    if((e->m_vertex[0]==center && e->m_vertex[1]==end) ||
       (e->m_vertex[1]==center && e->m_vertex[0]==end)) return e;
  }
  throw std::runtime_error("Missing subdivided edge lineage");
}
inline std::unique_ptr<ViboSubDEdgeSamples> edge_samples(ON_SubD& subd, unsigned density) {
  require(density>=1 && density<=6,"SubD edge density must be between 1 and 6");
  const uint64_t source_count=subd.EdgeCount();
  uint64_t v=subd.VertexCount(),e=source_count,f=subd.FaceCount(),c=0;
  auto fit=subd.FaceIterator();
  for(auto* face=fit.FirstFace();face;face=fit.NextFace()) c+=face->EdgeCount();
  for(unsigned i=0;i<density;++i) {
    const uint64_t nv=v+e+f,ne=2*e+c,nf=c;
    require(nv<=4000000 && ne<=4000000 && nf<=1000000,"SubD edge subdivision budget exceeded");
    v=nv;e=ne;f=nf;c=4*nf;
  }
  require(source_count*((uint64_t(1)<<density)+1)<=4000000,"SubD edge sample budget exceeded");
  std::vector<const ON_SubDEdge*> source;
  auto it=subd.EdgeIterator();
  for(auto* edge=it.FirstEdge();edge;edge=it.NextEdge()) source.push_back(edge);
  std::sort(source.begin(),source.end(),[](auto* a,auto* b){return a->m_id<b->m_id;});
  std::vector<EdgeTrack> tracks;
  tracks.reserve(source.size());
  for(auto* edge:source) tracks.push_back({edge,edge->m_vertex[0],edge->m_vertex[1]});
  for(unsigned level=0;level<density;++level) {
    require(subd.GlobalSubdivide(1),"SubD edge subdivision failed");
    std::vector<EdgeTrack> next;
    next.reserve(2*tracks.size());
    for(const auto& t:tracks) {
      const auto* a=t.start->m_subd_point1;
      const auto* b=t.end->m_subd_point1;
      const auto* m=t.edge->m_subd_point1;
      require(a && b && m && a!=b,"Invalid subdivided edge lineage");
      // The edge point's fan is bounded by its incident faces. Searching here
      // avoids rescanning high-valence original vertices for every edge.
      next.push_back({child_edge(m,a),a,m});
      next.push_back({child_edge(m,b),m,b});
    }
    tracks=std::move(next);
  }
  auto out=std::make_unique<ViboSubDEdgeSamples>();
  const size_t segments=size_t(1)<<density;
  out->offsets.reserve(source.size()+1);
  out->points.reserve(3*source.size()*(segments+1));
  for(size_t i=0;i<source.size();++i) {
    out->offsets.push_back(out->points.size()/3);
    for(size_t j=0;j<segments;++j) point(out->points,tracks[i*segments+j].start->SurfacePoint());
    point(out->points,tracks[(i+1)*segments-1].end->SurfacePoint());
  }
  out->offsets.push_back(out->points.size()/3);
  return out;
}
}
extern "C" int vibo_subd_edge_samples(const uint8_t* bytes,size_t count,unsigned density,
    ViboSubDEdgeSamples** out,char* error,size_t capacity) {
  return guard(out,error,capacity,[&]{
    auto subd=vibo_subd::decode(bytes,count);
    return vibo_subd::edge_samples(*subd,density);
  });
}
extern "C" int vibo_subd_edge_samples_view(const ViboSubDEdgeSamples* r,ViboSubDEdgeSamplesView* v) {
  if(!r || !v) return 0;
  *v={r->offsets.size()-1,r->points.size()/3,r->offsets.data(),r->points.data()};return 1;
}
extern "C" void vibo_subd_edge_samples_free(ViboSubDEdgeSamples* r) { delete r; }
#endif
