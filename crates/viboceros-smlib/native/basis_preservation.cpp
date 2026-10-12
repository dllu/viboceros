#include "basis_preservation.h"
#include <SmBSplineCurve.h>
#include <SmBSplineSurface.h>
#include <cmath>
#include <stdexcept>
#include <string>

namespace {
constexpr size_t limit = 1'000'000;
void require(bool ok, const char *message) {
    if (!ok) throw std::runtime_error(message);
}
void check(SmStatus status, const char *operation) {
    if (status != SM_SUCCESS)
        throw std::runtime_error(std::string(operation) + " status " + std::to_string(status));
}
void validate(const std::vector<double> &knots, size_t degree) {
    require(degree && knots.size() > 2 * degree + 1 && knots.size() <= limit + 65,
            "Invalid preserved basis size");
    for (size_t i = 0; i < knots.size(); ++i)
        require(std::isfinite(knots[i]) && (!i || knots[i] >= knots[i - 1]),
                "Invalid preserved basis knots");
    require(knots[degree] < knots[knots.size() - degree - 1], "Empty preserved basis interval");
}
void validate_controls(const std::vector<double> &controls, bool rational) {
    for (size_t i = 0; i < controls.size(); i += 4) {
        require(std::isfinite(controls[i]) && std::isfinite(controls[i + 1]) &&
                    std::isfinite(controls[i + 2]), "Invalid preserved controls");
        if (rational)
            require(std::isfinite(controls[i + 3]) && controls[i + 3] > 0,
                    "Invalid preserved control weights");
    }
}
}

std::vector<double> vb_curve_full_knots(const SmBSplineCurve &curve) {
    SmTArray<double> knots;
    check(curve.GetKnotsAll(knots), "read full curve basis");
    require(knots.GetDataArray() && knots.GetSize() && knots.GetSize() <= limit + 65,
            "Invalid native curve basis");
    return {knots.GetDataArray(), knots.GetDataArray() + knots.GetSize()};
}

std::array<std::vector<double>, 2> vb_surface_full_knots(const SmBSplineSurface &surface) {
    ULONG counts[2] = {0, 0};
    double *knots[2] = {nullptr, nullptr};
    check(surface.GetKnotsPointers(counts[0], counts[1], knots[0], knots[1]), "read full surface basis");
    std::array<std::vector<double>, 2> result;
    for (int d = 0; d < 2; ++d) {
        require(knots[d] && counts[d] && counts[d] <= limit + 65, "Invalid native surface basis");
        result[d].assign(knots[d], knots[d] + counts[d]);
    }
    return result;
}

bool vb_preserve_curve_basis(SmBSplineCurve &curve, const std::vector<double> &knots) {
    const auto degree = curve.GetDegree();
    validate(knots, degree);
    require(curve.GetOwner() == nullptr, "Basis restoration requires an unowned curve");
    const auto actual = vb_curve_full_knots(curve);
    require(actual.size() == knots.size(), "Native curve basis changed its control layout");
    if (actual == knots) return false;
    const auto count = knots.size() - degree - 1;
    require(count <= limit, "Preserved curve control limit");
    const bool rational = curve.IsRational();
    const auto form = rational ? SM_CP_HOMOGENEOUS_RATIONAL : SM_CP_NON_RATIONAL;
    std::vector<double> controls(count * 4);
    check(curve.GetControlPointsExpert(form, 0, count - 1, 4, controls.data()), "read preserved curve controls");
    validate_controls(controls, rational);
    check(curve.SetExpert(3, degree, SM_CF_UNSPECIFIED, SM_EK_CLAMPPED,
                          knots.size(), knots.data(), form, 4, controls.data()), "preserve curve basis");
    require(vb_curve_full_knots(curve) == knots, "Curve basis restoration failed");
    return true;
}

bool vb_preserve_surface_basis(SmBSplineSurface &surface,
                              const std::array<std::vector<double>, 2> &knots) {
    const auto pu = surface.GetDegree(SM_SP_U), pv = surface.GetDegree(SM_SP_V);
    validate(knots[0], pu);
    validate(knots[1], pv);
    require(surface.GetOwner() == nullptr, "Basis restoration requires an unowned surface");
    const auto actual = vb_surface_full_knots(surface);
    require(actual[0].size() == knots[0].size() && actual[1].size() == knots[1].size(),
            "Native surface basis changed its control layout");
    if (actual == knots) return false;
    const auto nu = knots[0].size() - pu - 1, nv = knots[1].size() - pv - 1;
    require(nu && nv && nu <= limit / nv, "Preserved surface control limit");
    const bool rational = surface.IsRational();
    const auto form = rational ? SM_CP_HOMOGENEOUS_RATIONAL : SM_CP_NON_RATIONAL;
    std::vector<double> controls(nu * nv * 4);
    // The header's stride description is reversed: the implementation requires
    // V as the inner dimension and validates U stride against the full V row.
    check(surface.GetControlPointsExpert(form, 0, nu - 1, 0, nv - 1, nv * 4, 4,
                                         controls.data()), "read preserved surface controls");
    validate_controls(controls, rational);
    check(surface.SetExpert(pu, pv, SM_SF_UNSPECIFIED, SM_EK_CLAMPPED,
                            knots[0].size(), knots[0].data(), knots[1].size(), knots[1].data(),
                            form, nv * 4, 4, controls.data()), "preserve surface basis");
    require(vb_surface_full_knots(surface) == knots, "Surface basis restoration failed");
    return true;
}
