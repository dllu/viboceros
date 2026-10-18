#include <SmApiGeneral.h>
#include <SmBSplineCurve.h>
#include <SmBSplineSurface.h>
#include "basis_preservation.h"
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <vector>

namespace {
void check(SmStatus status) {
    if (status != SM_SUCCESS) throw std::runtime_error("Native copy probe failed");
}
std::vector<double> curve_knots(const SmBSplineCurve &curve) {
    SmTArray<double> values;
    check(curve.GetKnotsAll(values));
    return {values.GetDataArray(), values.GetDataArray() + values.GetSize()};
}
std::vector<double> surface_knots(const SmBSplineSurface &surface, int axis) {
    ULONG nu, nv;
    double *u, *v;
    check(surface.GetKnotsPointers(nu, nv, u, v));
    return axis == 0 ? std::vector<double>(u, u + nu) : std::vector<double>(v, v + nv);
}
bool legal(const std::vector<double> &knots, size_t degree) {
    size_t run = 0;
    for (size_t i = 0; i < knots.size(); ++i) {
        run = i && knots[i] == knots[i - 1] ? run + 1 : 1;
        if (run > degree + 1) return false;
    }
    return knots[degree] < knots[knots.size() - degree - 1];
}
void array(const std::vector<double> &values) {
    std::cout << '[';
    for (size_t i = 0; i < values.size(); ++i) std::cout << (i ? "," : "") << values[i];
    std::cout << ']';
}
void report(const char *kind, int dimension_or_axis, int exponent, bool near_end,
            const std::vector<double> &input, const std::vector<double> &output,
            bool controls_match, bool source_unchanged) {
    std::cout << "{\"kind\":\"" << kind << "\",\"dimension_or_axis\":" << dimension_or_axis
              << ",\"exponent\":" << exponent << ",\"near_end\":" << near_end
              << ",\"knots_match\":" << (input == output) << ",\"controls_match\":" << controls_match
              << ",\"source_unchanged\":" << source_unchanged << ",\"legal_output\":" << legal(output, 2)
              << ",\"input_knots\":";
    array(input); std::cout << ",\"output_knots\":"; array(output); std::cout << "}\n";
}
}

int main() {
    try {
        std::cout << std::setprecision(17) << std::boolalpha;
        const auto &context = *SmApiGetOrCreateContext();
        for (int exponent : {-40, 0, 40}) for (bool near_end : {false, true}) {
            const double scale = std::ldexp(1., exponent);
            const double middle = near_end ? std::nextafter(scale, 0.) : 0.5 * scale;
            const std::vector<double> knots = {0, 0, 0, middle, middle, scale, scale, scale};
            for (int dimension : {2, 3}) {
                std::vector<double> controls;
                for (int i = 0; i < 5; ++i) {
                    const double w = i == 2 ? 0.5 : 1.;
                    controls.insert(controls.end(), {i * w, i * i * w,
                        dimension == 3 ? i * 0.25 * w : w, dimension == 3 ? w : 0.});
                }
                std::unique_ptr<SmBSplineCurve> source(new (context) SmBSplineCurve);
                check(source->SetExpert(dimension, 2, SM_CF_UNSPECIFIED, SM_EK_CLAMPPED,
                    knots.size(), knots.data(), SM_CP_HOMOGENEOUS_RATIONAL, 4, controls.data()));
                if (curve_knots(*source) != knots) throw std::runtime_error("Source curve basis changed");
                std::vector<double> original(controls.size());
                check(source->GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, 4, 4, original.data()));
                // 2D expert payloads use X,Y,W with one padding channel here;
                // 3D payloads use X,Y,Z,W. Compare the complete stored blocks.
                for (size_t i = 0; i < controls.size(); ++i)
                    if (original[i] != controls[i])
                        throw std::runtime_error("Source curve controls changed");
                SmCurve *raw = nullptr;
                check(source->Copy(context, raw));
                std::unique_ptr<SmCurve> copy(raw);
                auto &curve = *static_cast<SmBSplineCurve *>(copy.get());
                std::vector<double> actual(controls.size()), after(controls.size());
                check(curve.GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, 4, 4, actual.data()));
                check(source->GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, 4, 4, after.data()));
                report("curve", dimension, exponent, near_end, knots, curve_knots(curve),
                    actual == original && curve.GetDim() == source->GetDim(),
                    after == original && curve_knots(*source) == knots);
                auto scaled = knots;
                for (auto &k : scaled) k *= 2.;
                vb_preserve_curve_basis(curve, scaled);
                check(curve.GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, 4, 4, actual.data()));
                report("restored_curve", dimension, exponent, near_end, scaled, curve_knots(curve),
                    actual == original && curve.GetDim() == source->GetDim(),
                    curve_knots(*source) == knots);
            }
            for (int axis : {0, 1}) {
                const size_t nu = axis == 0 ? 5 : 2, nv = axis == 0 ? 2 : 5;
                const std::vector<double> other = {-3, -3, 4, 4};
                const auto &u = axis == 0 ? knots : other, &v = axis == 0 ? other : knots;
                std::vector<double> controls;
                for (size_t i = 0; i < nu; ++i) for (size_t j = 0; j < nv; ++j) {
                    const double x = double(i) / double(nu - 1), y = double(j) / double(nv - 1);
                    const double w = (i + j) % 3 == 1 ? 0.5 : 1.;
                    controls.insert(controls.end(), {x * w, y * w, x * y * w, w});
                }
                std::unique_ptr<SmBSplineSurface> source(new (context) SmBSplineSurface);
                check(source->SetExpert(axis == 0 ? 2 : 1, axis == 0 ? 1 : 2,
                    SM_SF_UNSPECIFIED, SM_EK_CLAMPPED, u.size(), u.data(), v.size(), v.data(),
                    SM_CP_HOMOGENEOUS_RATIONAL, nv * 4, 4, controls.data()));
                if (surface_knots(*source, axis) != knots) throw std::runtime_error("Source surface basis changed");
                SmSurface *raw = nullptr;
                check(source->Copy(context, raw));
                std::unique_ptr<SmSurface> copy(raw);
                auto &surface = *static_cast<SmBSplineSurface *>(copy.get());
                std::vector<double> actual(controls.size()), original(controls.size());
                check(surface.GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, nu - 1, 0, nv - 1, nv * 4, 4, actual.data()));
                check(source->GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, nu - 1, 0, nv - 1, nv * 4, 4, original.data()));
                report("surface", axis, exponent, near_end, knots, surface_knots(surface, axis),
                    actual == controls, original == controls && surface_knots(*source, axis) == knots);
            }
        }
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
