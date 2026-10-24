#ifndef VIBOCEROS_EXTRUSION_CAP_PROJECTION_H
#define VIBOCEROS_EXTRUSION_CAP_PROJECTION_H
#include <memory>
namespace vibo_extrusion {
// Work on a copy. A lossless common binary gauge removes the global profile
// scale before native end-weight reparameterization can amplify its condition.
inline bool balance_cap_weights(ON_NurbsCurve& curve) {
  if(!curve.IsRational()) return true;
  double maximum=0.;
  for(int i=0;i<curve.CVCount();++i) {
    const double w=curve.Weight(i);if(!(w>0.) || !std::isfinite(w)) return false;
    maximum=std::max(maximum,w);
  }
  if(!(maximum>0.)) return false;
  const int shift=-std::ilogb(maximum);
  for(int i=0;i<curve.CVCount();++i)for(int k=0;k<=curve.Dimension();++k) {
    const double value=curve.CV(i)[k],scaled=std::scalbn(value,shift);
    if(!std::isfinite(scaled) || std::scalbn(scaled,-shift)!=value) return false;
    curve.CV(i)[k]=scaled;
  }
  return true;
}

inline std::unique_ptr<ON_NurbsCurve> project_cap_curve(
    const ON_Brep& brep,const ON_BrepTrim& trim,const ON_BrepEdge& edge,
    const ON_PlaneSurface& plane,double& bound,double& scale) {
  auto curve=std::make_unique<ON_NurbsCurve>();
  if(edge.GetNurbForm(*curve)<=0 || !balance_cap_weights(*curve) ||
     !curve->MakePiecewiseBezier(true) || (trim.m_bRev3d && !curve->Reverse())) return {};
  if(curve->Weight(0)!=1. || curve->Weight(curve->CVCount()-1)!=1.) return {};
  for(int i=0;i<curve->CVCount();++i) {
    const double w=curve->Weight(i);ON_3dPoint p;
    if(!(w>0.) || !curve->GetCV(i,p)) return {};
    ON_3dPoint target=p;
    if(i==0 || i==curve->CVCount()-1) {
      const int vertex=trim.m_vi[i==0?0:1];
      if(vertex<0 || vertex>=brep.m_V.Count()) return {};
      target=brep.m_V[vertex].Point();
    }
    double s=0.,t=0.;
    if(!plane.m_plane.ClosestPointTo(target,&s,&t)) return {};
    const double u=plane.Domain(0).ParameterAt(plane.Extents(0).NormalizedParameterAt(s));
    const double v=plane.Domain(1).ParameterAt(plane.Extents(1).NormalizedParameterAt(t));
    if(!std::isfinite(u) || !std::isfinite(v) ||
       !curve->SetCV(i,ON_4dPoint(u*w,v*w,0.,w))) return {};
    ON_3dPoint uv;if(!curve->GetCV(i,uv)) return {};
    const auto q=plane.PointAt(uv.x,uv.y);
    if(!p.IsValid() || !q.IsValid()) return {};
    const double gap=std::hypot(p.x-q.x,p.y-q.y,p.z-q.z);
    if(!std::isfinite(gap)) return {};
    bound=std::max(bound,gap);
    for(unsigned k=0;k<3;++k) scale=std::max(scale,std::max(std::fabs(p[k]),std::fabs(q[k])));
  }
  // Both forward curves have identical positive weights and knots; their
  // maximum control displacement bounds the complete lifted trim error.
  curve->DestroyRuntimeCache();
  if(!curve->ChangeDimension(2) || !curve->SetDomain(trim.Domain()[0],trim.Domain()[1]) ||
     !curve->IsValid()) return {};
  return curve;
}
}
#endif
