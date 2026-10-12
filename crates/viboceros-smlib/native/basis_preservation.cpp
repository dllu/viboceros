#include "basis_preservation.h"
#include <SmBSplineCurve.h>
#include <SmBSplineSurface.h>
#include <SmNurbsCrv.h>
#include <nurbs.h>
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
            require(std::isfinite(controls[i + 3]) && controls[i + 3] != 0,
                    "Invalid preserved control weights");
    }
}

std::unique_ptr<SmBSplineSurface> surface_from_nlib(const SmContext &context, NL_SURFACE &surface) {
    NL_INDEX n, m, ku, kv;
    NL_DEGREE pu, pv;
    NL_CPOINT **controls;
    NL_REAL *u, *v;
    N_SrfGetCPtsDegreesAndKnots(&surface, &n, &m, &controls, &pu, &pv, &ku, &kv, &u, &v);
    require(n >= 0 && m >= 0 && n < 512 && m < 512, "Edge surface control-net resource limit");
    const std::array<std::vector<double>, 2> knots = {
        std::vector<double>(u, u + ku + 1), std::vector<double>(v, v + kv + 1)};
    std::unique_ptr<SmBSplineSurface> result(new (context) SmBSplineSurface(&surface));
    vb_preserve_surface_basis(*result, knots);
    return result;
}

void copy_curve_to_nlib(SmBSplineCurve &source, NL_CURVE &copy, NL_STACKS &stack) {
    N_CrvInitArrays(&copy);
    require(N_CrvCopy(source.GetOrCreateGwNurbPointer(), &copy, &stack) == NL_NO,
            "Copy native edge boundary failed");
}

void synchronize_edge_pair(NL_CURVE &a, NL_CURVE &b, NL_STACKS &stack) {
    NL_CURVE *curves[2] = {&a, &b};
    // The producer's default matcher averages near knots at domain/1000.
    // Synchronize via its existing explicit-tolerance operation first so
    // the tensor producer receives identical bases and preserves short spans.
    require(N_CrvsMakeCompatibleAdjKnots(curves, 1, 1e-300, &stack) == NL_NO,
            "Synchronize native edge boundary bases failed");
    require(a.pol->n < 512 && b.pol->n < 512, "Edge surface compatible control limit");
}

} // namespace

std::unique_ptr<SmBSplineSurface> vb_ruled_surface(SmBSplineCurve &first, SmBSplineCurve &second) {
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_CURVE a, b;
    copy_curve_to_nlib(first, a, stack);
    copy_curve_to_nlib(second, b, stack);
    synchronize_edge_pair(a, b, stack);
    NL_SURFACE surface;
    N_SrfInitArrays(&surface);
    require(N_CreateRuledSrf(&a, &b, NL_UDIR, &surface, &stack, &stack) == NL_NO,
            "Native ruled surface creation failed");
    return surface_from_nlib(*first.GetContext(), surface);
}

std::unique_ptr<SmBSplineSurface> vb_coons_surface(const std::array<SmBSplineCurve *, 4> &boundaries) {
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_CURVE curves[4];
    for (size_t i = 0; i < 4; ++i) {
        copy_curve_to_nlib(*boundaries[i], curves[i], stack);
        const auto interval = boundaries[i]->GetNaturalInterval();
        require(N_CrvReparamWeights(&curves[i], 1., interval.GetMin(), interval.GetMax()) == NL_NO,
                "Normalize native edge boundary end weights failed");
    }
    synchronize_edge_pair(curves[0], curves[1], stack);
    synchronize_edge_pair(curves[2], curves[3], stack);
    NL_CURVE *u[2] = {&curves[0], &curves[1]}, *v[2] = {&curves[2], &curves[3]};
    NL_SURFACE surface;
    N_SrfInitArrays(&surface);
    require(N_CreateCoonsSrf(u, v, &surface, &stack, &stack) == NL_NO,
            "Native Coons surface creation failed");
    return surface_from_nlib(*boundaries[0]->GetContext(), surface);
}

std::unique_ptr<SmBSplineSurface> vb_swapped_surface_uv(SmBSplineSurface &surface) {
    require(surface.GetOwner() == nullptr, "Parameter swap requires an unowned surface");
    const auto knots = vb_surface_full_knots(surface);
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_SURFACE copy;
    N_SrfInitArrays(&copy);
    require(N_SrfCopy(surface.GetOrCreateGwNurbPointer(), &copy, &stack) == NL_NO,
            "Copy native surface for parameter swap failed");
    require(N_SwapUV(&copy, &stack) == NL_NO, "Native surface parameter swap failed");
    std::unique_ptr<SmBSplineSurface> result(new (*surface.GetContext()) SmBSplineSurface(&copy));
    vb_preserve_surface_basis(*result, {knots[1], knots[0]});
    return result;
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
