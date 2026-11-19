#pragma once

#include "opennurbs_public.h"

#include <algorithm>
#include <cmath>
#include <limits>
#include <memory>
#include <set>

namespace vibo {

inline bool roundoff_sized_edge(const ON_BrepEdge& edge) {
  const auto box = edge.BoundingBox();
  if (!box.IsValid()) return false;
  double scale = 1.;
  for (int axis = 0; axis < 3; ++axis) {
    scale = std::max({scale, std::abs(box.m_min[axis]), std::abs(box.m_max[axis])});
  }
  return box.Diagonal().Length() <= 64. * std::numeric_limits<double>::epsilon() * scale;
}

inline bool negative_closed_trim_tangent(const ON_BrepTrim& trim) {
  const auto* edge = trim.Edge();
  const auto* face = trim.Face();
  if (!edge || !face || !face->SurfaceOf()) return false;
  const auto* surface = face->SurfaceOf();
  for (int end = 0; end < 2; ++end) {
    ON_3dPoint uv, point;
    ON_3dVector derivative, du, dv;
    if (!trim.Ev1Der(trim.Domain()[end], uv, derivative) ||
        !surface->Ev1Der(uv.x, uv.y, point, du, dv)) continue;
    auto direction = derivative.x * du + derivative.y * dv;
    if (!direction.Unitize()) continue;
    const int edge_end = end ^ static_cast<int>(trim.m_bRev3d);
    const double dot = direction * edge->TangentAt(edge->Domain()[edge_end]);
    if ((trim.m_bRev3d ? -dot : dot) < 0.) return true;
  }
  return false;
}

// Sewing can unify the endpoint vertices of a carried pole edge whose entire
// spatial extent is roundoff. OpenNURBS then interprets its numerical tangent
// as the orientation of a closed curve. An exact chart transpose preserves
// the physical surface, loop loci, face sense and shared edge topology.
inline std::unique_ptr<ON_Brep> try_rechart_roundoff_poles(const ON_Brep& source) {
  std::set<int> faces;
  for (int index = 0; index < source.m_E.Count(); ++index) {
    const auto& edge = source.m_E[index];
    if (edge.m_vi[0] != edge.m_vi[1] || edge.m_ti.Count() != 1 ||
        !roundoff_sized_edge(edge)) continue;
    const auto& trim = source.m_T[edge.m_ti[0]];
    if (negative_closed_trim_tangent(trim)) faces.insert(trim.Face()->m_face_index);
  }
  if (faces.empty()) return nullptr;
  auto result = std::make_unique<ON_Brep>(source);
  for (const int face : faces) {
    if (!result->m_F[face].Transpose()) return nullptr;
  }
  if (!result->SetTrimIsoFlags() || !result->IsValid()) return nullptr;
  return result;
}

} // namespace vibo
