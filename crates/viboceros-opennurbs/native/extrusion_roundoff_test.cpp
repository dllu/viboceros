// Independent fault checks for the cap tolerance repair; 3D geometry stays intact.
#include "opennurbs_public.h"
#include "extrusion_roundoff.h"
#include <cstdio>
#include <memory>
#include <vector>

namespace {
std::vector<ON__UINT32> model_geometry_crc(const ON_Brep& brep) {
  std::vector<ON__UINT32> crc;
  for(int i=0;i<brep.m_C3.Count();++i) crc.push_back(brep.m_C3[i]->DataCRC(0));
  for(int i=0;i<brep.m_S.Count();++i) crc.push_back(brep.m_S[i]->DataCRC(0));
  return crc;
}
int check() {
  ON_NurbsCurve curve;ON_Circle(ON_xy_plane,2.).GetNurbForm(curve);
  for(int i=0;i<curve.CVCount();++i)for(int k=0;k<4;++k)curve.CV(i)[k]*=7.;
  // The native cap closes this rounding-sized gap and unsets edge tolerances.
  curve.CV(curve.CVCount()-1)[1]=7.*1e-16;
  auto extrusion=std::unique_ptr<ON_Extrusion>(
      ON_Extrusion::CreateFrom3dCurve(curve,&ON_xy_plane,2.,true));
  if(!extrusion) return 1;
  std::unique_ptr<ON_Brep> good(extrusion->BrepForm(nullptr,true));
  if(!good || good->IsValid() || !good->IsValidTopology() || !good->IsValidGeometry()) return 2;
  const auto original=model_geometry_crc(*good);
  ON_Brep bad(*good);std::vector<int> displaced_edges;
  std::vector<std::pair<int,int>> displaced_trims;
  std::vector<ON__UINT32> displaced_trim_crc;
  for(int i=0;i<bad.m_F.Count();++i) {
    auto& face=bad.m_F[i];
    const auto* plane=ON_PlaneSurface::Cast(face.SurfaceOf());
    if(!plane) continue;
    auto moved=std::unique_ptr<ON_PlaneSurface>(plane->Duplicate());
    ON_Xform translation;translation.Translation(0.,0.,1e-4);
    if(!moved->Transform(translation)) return 3;
    for(int l=0;l<face.m_li.Count();++l) {
      const auto& loop=bad.m_L[face.m_li[l]];
      for(int t=0;t<loop.m_ti.Count();++t) {
        const int edge=bad.m_T[loop.m_ti[t]].m_ei;
        if(edge<0) return 4;
        displaced_edges.push_back(edge);bad.m_E[edge].m_tolerance=ON_UNSET_VALUE;
        displaced_trims.emplace_back(loop.m_ti[t],bad.m_T[loop.m_ti[t]].m_c2i);
        displaced_trim_crc.push_back(bad.m_C2[bad.m_T[loop.m_ti[t]].m_c2i]->DataCRC(0));
      }
    }
    face.m_si=bad.AddSurface(moved.release());face.SetProxySurface(bad.m_S[face.m_si]);
    face.DestroyRuntimeCache();break;
  }
  if(displaced_edges.empty() || !bad.IsValidTopology() || !bad.IsValidGeometry()) return 5;
  const auto displaced=model_geometry_crc(bad);
  vibo_extrusion::bound_cap_roundoff(*good);vibo_extrusion::bound_cap_roundoff(bad);
  if(!good->IsValid() || model_geometry_crc(*good)!=original) return 6;
  for(int i=0;i<good->m_L.Count();++i) {
    const auto& loop=good->m_L[i];
    for(int j=0;j<loop.m_ti.Count();++j)
      if(good->m_T[loop.m_ti[j]].PointAtEnd()!=good->m_T[loop.m_ti[(j+1)%loop.m_ti.Count()]].PointAtStart()) return 10;
  }
  if(bad.IsValid() || model_geometry_crc(bad)!=displaced) return 7;
  for(size_t i=0;i<displaced_trims.size();++i) {
    const auto& trim=displaced_trims[i];
    if(bad.m_T[trim.first].m_c2i!=trim.second ||
       bad.m_C2[trim.second]->DataCRC(0)!=displaced_trim_crc[i]) return 9;
  }
  for(int edge:displaced_edges) if(bad.m_E[edge].m_tolerance!=ON_UNSET_VALUE) return 8;
  return 0;
}
}
int main() {
  ON::Begin();const int result=check();ON::End();
  std::printf("Cap roundoff repair and displaced-cap rejection: %s (%d)\n",
              result==0?"passed":"failed",result);
  return result;
}
