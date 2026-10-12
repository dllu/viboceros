#pragma once
#include <array>
#include <memory>
#include <vector>
class SmBSplineCurve;
class SmBSplineSurface;

// Call under the bridge's kernel lock, on independently owned geometry.
std::vector<double> vb_curve_full_knots(const SmBSplineCurve &curve);
std::array<std::vector<double>, 2> vb_surface_full_knots(const SmBSplineSurface &surface);
bool vb_preserve_curve_basis(SmBSplineCurve &curve, const std::vector<double> &knots);
bool vb_preserve_surface_basis(SmBSplineSurface &surface,
                              const std::array<std::vector<double>, 2> &knots);
// Existing NLib copy/transpose operations avoid canonical knot grouping and
// canonical-import weight restrictions. The returned surface is independently owned.
std::unique_ptr<SmBSplineSurface> vb_swapped_surface_uv(SmBSplineSurface &surface);
// NLib producers with full output knot arrays retained before construction.
std::unique_ptr<SmBSplineSurface> vb_ruled_surface(SmBSplineCurve &first, SmBSplineCurve &second);
std::unique_ptr<SmBSplineSurface> vb_coons_surface(const std::array<SmBSplineCurve *, 4> &boundaries);
