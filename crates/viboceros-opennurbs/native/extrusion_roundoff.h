#ifndef VIBOCEROS_EXTRUSION_ROUNDOFF_H
#define VIBOCEROS_EXTRUSION_ROUNDOFF_H
#include <algorithm>
#include <cmath>
#include <limits>
#include "extrusion_cap_projection.h"
namespace vibo_extrusion {
// A plane is affine. When an edge and a lifted trim have the same positive
// rational basis, their pointwise distance is bounded by the largest control
// displacement. This supplies a geometric tolerance for native cap roundoff;
// OpenNURBS' public SetEdgeTolerance does not implement this calculation.
inline void bound_cap_roundoff(ON_Brep& brep) {
  if(!brep.IsValidTopology() || !brep.IsValidGeometry()) return;
  size_t work=0;
  for(int trim_index=0;trim_index<brep.m_T.Count();++trim_index) {
    auto& trim=brep.m_T[trim_index];
    auto* edge=trim.Edge();const auto* face=trim.Face();
    const auto* plane=face?ON_PlaneSurface::Cast(face->SurfaceOf()):nullptr;
    if(!edge || !plane) continue;
    ON_NurbsCurve c2,c3;
    if(trim.GetNurbForm(c2)<=0 || edge->GetNurbForm(c3)<=0 ||
       c2.Order()!=c3.Order() || c2.Order()<2) continue;
    // MakeCap2dCurvesHelper uses this public reparameterization for cap trims.
    // Apply it to both temporary copies so scaled profile weights do not leave
    // geometrically identical boundaries in different positive rational bases.
    const auto budget=[](const ON_NurbsCurve& curve) {
      return uint64_t(curve.SpanCount())*uint64_t(curve.Degree())+1<=4000000;
    };
    if(!budget(c2) || !budget(c3) || !c2.MakePiecewiseBezier(true) ||
       !c3.MakePiecewiseBezier(true) || c2.CVCount()!=c3.CVCount() ||
       c2.KnotCount()!=c3.KnotCount()) continue;
    work+=c2.CVCount();if(work>4000000) return;
    if(trim.m_bRev3d && !c2.Reverse()) continue;
    if(!c2.SetDomain(0.,1.) || !c3.SetDomain(0.,1.)) continue;
    bool common=true;
    for(int i=0;i<c2.KnotCount();++i) if(c2.Knot(i)!=c3.Knot(i)) {common=false;break;}
    if(!common) continue;
    double bound=0.,scale=0.;
    for(int i=0;i<c2.CVCount();++i) {
      const double w=c2.Weight(i);
      if(!(w>0.) || w!=c3.Weight(i)) {common=false;break;}
      ON_3dPoint uv,p;
      if(!c2.GetCV(i,uv) || !c3.GetCV(i,p)) {common=false;break;}
      const auto q=face->PointAt(uv.x,uv.y);
      if(!p.IsValid() || !q.IsValid()) {common=false;break;}
      const double gap=std::hypot(p.x-q.x,p.y-q.y,p.z-q.z);
      if(!std::isfinite(gap)) {common=false;break;}
      bound=std::max(bound,gap);
      for(unsigned k=0;k<3;++k) scale=std::max(scale,std::max(std::fabs(p[k]),std::fabs(q[k])));
    }
    // Restrict this repair to rounding-sized discrepancies. Larger geometric
    // differences still fail the shared validator, with its native diagnostic.
    if(!common || !(scale>0.) || bound>64.*std::numeric_limits<double>::epsilon()*scale) continue;
    for(unsigned k=0;k<2;++k) {
      const auto uv=trim.PointAt(trim.Domain()[k]);
      const auto p=face->PointAt(uv.x,uv.y);
      const auto q=edge->PointAt(edge->Domain()[trim.m_bRev3d?1-k:k]);
      if(!p.IsValid() || !q.IsValid()) {common=false;break;}
      bound=std::max(bound,std::hypot(p.x-q.x,p.y-q.y,p.z-q.z));
    }
    if(!common || !std::isfinite(bound) || bound>64.*std::numeric_limits<double>::epsilon()*scale) continue;
    // The cap's original parameterization can be ill conditioned solely from
    // a global profile weight scale. Rebuild only its derived 2D representation
    // after the existing boundary and the projected replacement are bounded.
    auto projected=project_cap_curve(brep,trim,*edge,*plane,bound,scale);
    if(!projected || !std::isfinite(bound) || bound>64.*std::numeric_limits<double>::epsilon()*scale) continue;
    const int curve=brep.AddTrimCurve(projected.release());
    if(!trim.ChangeTrimCurve(curve)) continue;
    // Protect the computed bound from its final floating-point rounding.
    const double tolerance=std::nextafter(1.001*bound,std::numeric_limits<double>::infinity());
    if(std::isfinite(tolerance)) edge->m_tolerance=std::max(edge->m_tolerance,tolerance);
  }
}
}
#endif
