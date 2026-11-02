#ifndef VIBOCEROS_SUBD_CURVATURE_H
#define VIBOCEROS_SUBD_CURVATURE_H
// Public bicubic limit patches provide exact regular second derivatives.
namespace vibo_subd {
class JetSamples {
  std::map<std::pair<const ON_SubDVertex*,const ON_SubDFace*>,uint32_t> cache;
  static bool evaluate(const ON_SubDFace* face,const ON_SubDVertex* vertex,double* jet) {
    ON_NurbsSurface patch;
    if(!face->GetQuadSurface(patch)) return false;
    unsigned corner=0;
    while(corner<4 && face->Vertex(corner)!=vertex) ++corner;
    if(corner==4) return false;
    const double uv[4][2]={{0,0},{1,0},{1,1},{0,1}};
    if(!patch.Evaluate(uv[corner][0],uv[corner][1],2,3,jet)) return false;
    return std::all_of(jet,jet+18,[](double v){return std::isfinite(v);});
  }
public:
  std::vector<double> jets;
  std::vector<uint32_t> corners;
  void append(const ON_SubDFace* face,unsigned corner) {
    auto* vertex=face->Vertex(corner);
    ON_SubDSectorSurfacePoint point=ON_SubDSectorSurfacePoint::Nan;
    const auto* sector=vertex->GetSurfacePoint(face,true,point)?point.m_sector_face:face;
    const auto key=std::make_pair(vertex,sector);
    const auto found=cache.find(key);
    if(found!=cache.end()) {corners.push_back(found->second);return;}
    if(vertex->m_vertex_tag==ON_SubDVertexTag::Smooth && vertex->EdgeCount()!=4) {
      cache.emplace(key,UINT32_MAX);corners.push_back(UINT32_MAX);return;
    }
    double jet[18];bool valid=evaluate(face,vertex,jet);
    // Search only this vertex sector; never borrow derivatives across a crease.
    for(unsigned direction=0;!valid && direction<2;++direction) {
      ON_SubDSectorIterator iterator;
      if(!iterator.Initialize(face,direction,vertex)) break;
      for(unsigned i=0;i<vertex->FaceCount();++i) {
        const auto* candidate=iterator.NextFace(ON_SubDSectorIterator::StopAt::AnyCrease);
        if(!candidate || candidate==face) break;
        if(evaluate(candidate,vertex,jet)) {valid=true;break;}
      }
    }
    const auto index=valid?static_cast<uint32_t>(jets.size()/18):UINT32_MAX;
    if(valid) jets.insert(jets.end(),jet,jet+18);
    cache.emplace(key,index);corners.push_back(index);
  }
};
}
#endif
