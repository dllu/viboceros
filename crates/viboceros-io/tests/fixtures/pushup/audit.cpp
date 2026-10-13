// Independent VIB-02 archive audit through the public OpenNURBS API.
// This executable does not link the Viboceros reader or geometry kernel.
#include "opennurbs.h"
#include <algorithm>
#include <cmath>
#include <iostream>

static ON_3dPoint ellipse(double parameter, double offset) {
  static const double controls[][2] = {
      {0.75, 0.5}, {0.75, 0.625}, {0.5, 0.625}, {0.25, 0.625},
      {0.25, 0.5}, {0.25, 0.375}, {0.5, 0.375}, {0.75, 0.375},
      {0.75, 0.5}};
  const int quadrant = std::min(3, static_cast<int>(parameter));
  const double t = parameter - quadrant;
  const double basis[] = {(1.0 - t) * (1.0 - t),
                          2.0 * t * (1.0 - t) * std::sqrt(0.5), t * t};
  double u = 0.0, v = 0.0, weight = 0.0;
  for (int i = 0; i < 3; ++i) {
    u += basis[i] * controls[2 * quadrant + i][0];
    v += basis[i] * controls[2 * quadrant + i][1];
    weight += basis[i];
  }
  u /= weight;
  v /= weight;
  return ON_3dPoint(u, v, u * v + offset);
}

int main(int argc, char** argv) {
  if (argc != 2) return 2;
  ON::Begin();
  ONX_Model model;
  ON_TextLog log(stdout);
  if (!model.Read(argv[1], &log)) return 3;
  ONX_ModelComponentIterator objects(model, ON_ModelComponent::Type::ModelGeometry);
  int count = 0;
  double worst = 0.0;
  for (auto reference = objects.FirstComponentReference(); !reference.IsEmpty();
       reference = objects.NextComponentReference()) {
    const auto* component = ON_ModelGeometryComponent::FromModelComponentRef(reference, nullptr);
    const auto* brep = component ? ON_Brep::Cast(component->Geometry(nullptr)) : nullptr;
    if (!brep || !brep->IsValid(&log) || !brep->IsSolid() ||
        brep->m_F.Count() != 3 || brep->m_E.Count() != 9 ||
        brep->m_V.Count() != 8 || brep->m_T.Count() != 18) return 4;
    ++count;
    for (int i = 0; i < brep->m_E.Count(); ++i) {
      const ON_BrepEdge& edge = brep->m_E[i];
      if (!edge.IsValid(&log) || edge.m_ti.Count() != 2) return 5;
      const ON_Curve* curve = edge.EdgeCurveOf();
      const auto* nurbs = ON_NurbsCurve::Cast(curve);
      if (!nurbs || !nurbs->IsValid(&log)) return 6;
      const ON_Interval domain = edge.Domain();
      const bool seam = nurbs->Degree() == 1;
      const double offset = seam ? 0.0
          : (edge.PointAtStart().DistanceTo(ellipse(domain[0], 0.0)) < 1e-12
              ? 0.0 : -0.125);
      for (int sample = 0; sample <= 64; ++sample) {
        const double t = domain.ParameterAt(sample / 64.0);
        const ON_3dPoint expected = seam
            ? ON_3dPoint(0.75, 0.5, 0.375 - t)
            : ellipse(t, offset);
        worst = std::max(worst, edge.PointAt(t).DistanceTo(expected));
      }
    }
    for (int i = 0; i < brep->m_T.Count(); ++i) {
      const ON_BrepTrim& trim = brep->m_T[i];
      const ON_BrepEdge* edge = trim.Edge();
      const ON_Surface* surface = trim.SurfaceOf();
      if (!edge || !surface || !trim.IsValid(&log)) return 7;
      for (int sample = 0; sample <= 64; ++sample) {
        const double fraction = sample / 64.0;
        const ON_3dPoint uv = trim.PointAt(trim.Domain().ParameterAt(fraction));
        const ON_3dPoint image = surface->PointAt(uv.x, uv.y);
        const double t = edge->Domain().ParameterAt(trim.m_bRev3d ? 1.0 - fraction : fraction);
        worst = std::max(worst, image.DistanceTo(edge->PointAt(t)));
      }
    }
  }
  if (count != 1 || worst > 1e-12) return 8;
  std::cout << "valid closed panel: 3 faces, 9 edges; maximum deviation " << worst << '\n';
  ON::End();
  return 0;
}
