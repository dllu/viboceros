// Original regression fixtures created only through public OpenNURBS APIs.
#include "opennurbs.h"
#include <memory>

int main(int argc, char** argv) {
  if (argc != 2) return 2;
  ON::Begin();
  ONX_Model model;
  ON_Layer layer;
  layer.SetName(L"Derived isocurve flags");
  if (model.AddModelComponent(layer, true).IsEmpty()) return 3;
  const ON_3dPoint corners[] = {
      ON_3dPoint(0,0,0), ON_3dPoint(1,0,0), ON_3dPoint(1,1,0), ON_3dPoint(0,1,0),
      ON_3dPoint(0,0,1), ON_3dPoint(1,0,1), ON_3dPoint(1,1,1), ON_3dPoint(0,1,1)};
  std::unique_ptr<ON_Brep> box(ON_BrepBox(corners));
  ON_Polyline polygon;
  polygon.Append(ON_3dPoint(0,0,0));
  polygon.Append(ON_3dPoint(1,0,0));
  polygon.Append(ON_3dPoint(0,1,0));
  polygon.Append(ON_3dPoint(0,0,0));
  ON_PolylineCurve boundary(polygon);
  std::unique_ptr<ON_Brep> triangle(ON_BrepTrimmedPlane(ON_Plane::World_xy, boundary));
  if (!box || !triangle || !box->IsValid() || !triangle->IsValid()) return 4;
  box->m_T[0].m_iso = ON_Surface::not_iso; // Should be a surface boundary iso.
  bool changed = false;
  for (int i = 0; i < triangle->m_T.Count(); ++i) {
    if (triangle->m_T[i].m_iso == ON_Surface::not_iso) {
      triangle->m_T[i].m_iso = ON_Surface::S_iso; // Should be not_iso (diagonal).
      changed = true;
      break;
    }
  }
  if (!changed || box->IsValid() || triangle->IsValid()) return 5;
  for (int i = 0; i < 2; ++i) {
    auto* attributes = new ON_3dmObjectAttributes;
    attributes->m_layer_index = 0;
    attributes->m_uuid = ON_nil_uuid;
    attributes->m_uuid.Data1 = 201 + i;
    attributes->SetName(i == 0 ? L"stale boundary iso" : L"stale diagonal iso", true);
    ON_Brep* brep = i == 0 ? box.release() : triangle.release();
    if (model.AddManagedModelGeometryComponent(brep, attributes, false).IsEmpty()) return 6;
  }
  const bool written = model.Write(argv[1], 80, nullptr);
  ON::End();
  return written ? 0 : 7;
}
