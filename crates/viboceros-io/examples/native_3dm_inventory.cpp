// Optional independent inspection of objects unsupported by the portable reader.
#include "opennurbs.h"
#include <algorithm>
#include <iomanip>
#include <iostream>
#include <string>

int main(int argc, char** argv) {
  if (argc < 2 || argc > 3) return 2;
  ON::Begin();
  ONX_Model model;
  if (!model.Read(argv[1], nullptr)) return 3;
  ONX_ModelComponentIterator objects(model, ON_ModelComponent::Type::ModelGeometry);
  int index = 0;
  for (auto reference = objects.FirstComponentReference(); !reference.IsEmpty();
       reference = objects.NextComponentReference(), ++index) {
    const auto* component = ON_ModelGeometryComponent::FromModelComponentRef(reference, nullptr);
    const auto* attributes = component ? component->Attributes(nullptr) : nullptr;
    const ON_String name = attributes ? ON_String(attributes->Name()) : ON_String("");
    if (argc == 3 && std::string(name.Array()).find(argv[2]) == std::string::npos) continue;
    const auto* brep = component ? ON_Brep::Cast(component->Geometry(nullptr)) : nullptr;
    if (!brep) continue;
    ON_wString diagnostics;
    ON_TextLog log(diagnostics);
    const bool valid = brep->IsValid(&log);
    std::cout << "object " << index << " " << std::quoted(name.Array())
              << " valid=" << valid << " faces=" << brep->m_F.Count()
              << " edges=" << brep->m_E.Count() << " trims=" << brep->m_T.Count() << '\n';
    if (!valid) std::cout << ON_String(diagnostics).Array() << '\n';
    ON_Brep normalized(*brep);
    normalized.SetTolerancesBoxesAndFlags(true, true, true, true, false, false, false, false);
    diagnostics = L"";
    const bool normalized_valid = normalized.IsValid(&log);
    std::cout << "  normalized_valid=" << normalized_valid << '\n';
    if (!normalized_valid) std::cout << ON_String(diagnostics).Array() << '\n';
    normalized.SetTrimIsoFlags();
    diagnostics = L"";
    const bool reclassified_valid = normalized.IsValid(&log);
    std::cout << "  reclassified_valid=" << reclassified_valid << '\n';
    if (!reclassified_valid) std::cout << ON_String(diagnostics).Array() << '\n';
    for (int face = 0; face < brep->m_F.Count(); ++face) {
      ON_NurbsSurface surface;
      const int form = brep->m_F[face].GetNurbForm(surface);
      std::cout << "  face " << face << " nurb_form=" << form
                << " valid=" << surface.IsValid() << " controls=" << surface.CVCount(0)
                << ',' << surface.CVCount(1) << " knots=" << surface.KnotCount(0)
                << ',' << surface.KnotCount(1) << " spans=" << surface.SpanCount(0)
                << ',' << surface.SpanCount(1) << '\n';
    }
    for (int trim = 0; trim < brep->m_T.Count(); ++trim) {
      ON_NurbsCurve curve;
      const int form = brep->m_T[trim].GetNurbForm(curve);
      if (form < 1 || !curve.IsValid() || curve.CVCount() > 100)
        std::cout << "  trim " << trim << " nurb_form=" << form
                  << " valid=" << curve.IsValid() << " controls=" << curve.CVCount()
                  << " knots=" << curve.KnotCount() << " spans=" << curve.SpanCount() << '\n';
    }
  }
  ON::End();
  return 0;
}
