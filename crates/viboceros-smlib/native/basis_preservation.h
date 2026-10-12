#pragma once
#include <array>
#include <vector>
class SmBSplineCurve;
class SmBSplineSurface;

// Call under the bridge's kernel lock, on independently owned geometry.
std::vector<double> vb_curve_full_knots(const SmBSplineCurve &curve);
std::array<std::vector<double>, 2> vb_surface_full_knots(const SmBSplineSurface &surface);
bool vb_preserve_curve_basis(SmBSplineCurve &curve, const std::vector<double> &knots);
bool vb_preserve_surface_basis(SmBSplineSurface &surface,
                              const std::array<std::vector<double>, 2> &knots);
