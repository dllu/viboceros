#include "brep_data.h"
#include <SmApiCurves.h>
#include <SmApiGeneral.h>
#include <SmBSplineCurve.h>
#include <SmBSplineSurface.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <vector>

namespace {
void check(SmStatus status) {
    if (status != SM_SUCCESS) throw std::runtime_error(std::to_string(status));
}
using Point = std::array<double, 3>;
Point world(Point point, double scale, int axis) {
    Point result{};
    for (int i = 0; i < 3; ++i)
        result[(i + axis) % 3] = scale * (point[i] + (axis == 1 ? 32 : 0));
    return result;
}
std::vector<double> payload(VbSurfaceData *raw) {
    std::unique_ptr<VbSurfaceData, decltype(&vb_surface_data_free)> owned(raw, vb_surface_data_free);
    VbNurbsData data{};
    if (!raw || vb_surface_data_view(raw, &data)) throw std::runtime_error("export NURBS");
    std::vector<double> result;
    for (int axis = 0; axis < 2; ++axis) {
        result.push_back(data.degree[axis]);
        result.push_back(data.count[axis]);
        result.push_back(data.knot_count[axis]);
        if (data.knot_count[axis])
            result.insert(result.end(), data.knots[axis], data.knots[axis] + data.knot_count[axis]);
    }
    result.insert(result.end(), data.points_xyzw, data.points_xyzw + 4 * data.count[0] * data.count[1]);
    return result;
}
double bump(double r) {
    r = std::abs(r);
    return r < 1 ? 2. / 3 - r * r + .5 * r * r * r
                 : r < 2 ? std::pow(2 - r, 3) / 6 : 0;
}
}

// Independent analytic family: S(u,v)=(u,v+a*beta3((u-c)/h),u^2),
// C(t)=(t,.5,t^2). The ideal UV is (t,.5-a*beta3((t-c)/h)).
// Its lateral error is a lower bound on distance to the entire source curve.
int main() {
    try {
        std::cout << std::setprecision(17) << std::boolalpha;
        size_t cases = 0, failures = 0;
        for (double center : {.37, .5, .73})
        for (double width : {.01, .001, .0001, .00001, .000001})
        for (double scale : {.001, 1., 1000.})
        for (int axis : {0, 1, 2})
        for (bool reverse : {false, true}) {
            ++cases;
            const double amplitude = .001, tolerance = 1e-8 * scale;
            std::vector<double> u(4, 0.), v{0, 0, 1, 1}, points;
            for (int j = -2; j <= 2; ++j) u.push_back(center + j * width);
            u.insert(u.end(), 4, 1.);
            const size_t nu = u.size() - 4;
            for (int j = 0; j < 2; ++j) for (size_t i = 0; i < nu; ++i) {
                const double a = u[i + 1], b = u[i + 2], c = u[i + 3];
                const auto p = world({(a + b + c) / 3., j + (i == 4 ? amplitude : 0.),
                                      (a * b + a * c + b * c) / 3.}, scale, axis);
                points.insert(points.end(), {p[0], p[1], p[2], 1.});
            }
            const VbNurbsData definition{{3, 1}, {nu, 2}, {u.size(), v.size()},
                                          {u.data(), v.data()}, points.data()};
            std::unique_ptr<SmSurface> surface(vb_import_query_surface(definition));
            SmTArray<SmPoint3d> controls;
            for (int i = 0; i < 3; ++i) {
                const int k = reverse ? 2 - i : i;
                const auto p = world({k * .5, .5, k == 2 ? 1. : 0.}, scale, axis);
                controls.Add(SmPoint3d(p[0], p[1], p[2]));
            }
            SmTArray<double> knots; knots.Add(0); knots.Add(1);
            SmTArray<ULONG> multiplicities; multiplicities.Add(3); multiplicities.Add(3);
            SmBSplineCurve *raw = nullptr;
            check(SmApiCreateCanonicalCurve(controls, knots, multiplicities, nullptr, 2,
                                            SM_CF_UNSPECIFIED, raw));
            std::unique_ptr<SmBSplineCurve> curve(raw);
            auto *spline = dynamic_cast<SmBSplineSurface *>(surface.get());
            if (!spline) throw std::runtime_error("expected spline surface");
            const auto before_surface = payload(vb_export_surface(*spline));
            const auto before_curve = payload(vb_export_curve(*curve));
            SmTArray<SmBSplineCurve *> uv;
            double drop = 0, deviation = 0;
            const auto status = surface->DropCurve(*SmApiGetOrCreateContext(),
                surface->GetNaturalUVDomain(), *curve, curve->GetNaturalInterval(),
                tolerance, drop, deviation, uv, FALSE);
            std::vector<std::unique_ptr<SmBSplineCurve>> output;
            for (ULONG i = 0; i < uv.GetSize(); ++i) output.emplace_back(uv[i]);
            const bool unchanged = before_surface == payload(vb_export_surface(*spline))
                                && before_curve == payload(vb_export_curve(*curve));
            // Dropped UV curves must remain independently owned after their inputs die.
            curve.reset(); surface.reset();
            size_t control_count = 0, samples = 0;
            double maximum = 0, worst_parameter = 0;
            bool finite = true, bounded = true;
            for (const auto &item : output) {
                control_count += item->GetNumberControlPoints();
                SmTArray<double> breaks; check(item->GetKnots(breaks));
                const auto sample = [&](double t) {
                    SmPoint3d q; check(item->EvaluatePoint(t, q));
                    const double error = scale * std::abs(q.y + amplitude * bump((q.x - center) / width) - .5);
                    finite &= std::isfinite(error) && std::isfinite(q.x) && std::isfinite(q.y);
                    bounded &= q.x >= -1e-12 && q.x <= 1 + 1e-12 && q.y >= 0 && q.y <= 1;
                    if (error > maximum) { maximum = error; worst_parameter = t; }
                    ++samples;
                };
                // Uniform stations on every output span plus stations on the
                // independent feature chart, which also catch a false straight UV line.
                for (ULONG j = 0; j + 1 < breaks.GetSize(); ++j)
                    for (int k = 0; k <= 64; ++k)
                        sample(breaks[j] + (breaks[j + 1] - breaks[j]) * k / 64.);
                for (int k = -128; k <= 128; ++k) {
                    const double x = center + width * k / 64.;
                    sample(reverse ? 1 - x : x);
                }
            }
            const bool passed = status == SM_SUCCESS && output.size() == 1 && unchanged
                             && finite && bounded && maximum <= tolerance;
            failures += !passed;
            std::cout << "{\"center\":" << center << ",\"width\":" << width
                      << ",\"scale\":" << scale << ",\"axis\":" << axis
                      << ",\"reverse\":" << reverse << ",\"tolerance\":" << tolerance
                      << ",\"status\":" << status << ",\"curves\":" << output.size()
                      << ",\"control_points\":" << control_count << ",\"samples\":" << samples
                      << ",\"reported_drop\":" << drop << ",\"reported_deviation\":" << deviation
                      << ",\"maximum_lateral_error\":" << maximum
                      << ",\"worst_parameter\":" << worst_parameter
                      << ",\"source_unchanged\":" << unchanged << ",\"finite\":" << finite
                      << ",\"bounded\":" << bounded << ",\"passed\":" << passed << "}\n";
        }
        std::cerr << cases << " curve drop cases, " << failures << " failures\n";
        return failures ? 1 : 0;
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 2;
    }
}
