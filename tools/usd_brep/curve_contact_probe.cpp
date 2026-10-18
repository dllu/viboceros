#include "bridge.h"
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <vector>

namespace {
using Point = std::array<double, 3>;
struct Curve {
    size_t degree;
    std::vector<double> points, knots;
    bool operator==(const Curve &b) const {
        return degree == b.degree && points == b.points && knots == b.knots;
    }
};

Point world(Point p, double scale, int axis) {
    Point result{};
    const double origin = axis == 1 ? 32 * scale : 0;
    for (size_t i = 0; i < 3; ++i) result[(i + axis) % 3] = origin + scale * p[i];
    return result;
}
Curve definition(size_t degree, const std::vector<Point> &points,
                 const std::vector<double> &weights, double scale, int axis) {
    Curve result{degree, {}, std::vector<double>(2 * (degree + 1), 1.)};
    std::fill_n(result.knots.begin(), degree + 1, 0.);
    for (size_t i = 0; i < points.size(); ++i) {
        const auto p = world(points[i], scale, axis);
        result.points.insert(result.points.end(), {p[0], p[1], p[2], weights[i]});
    }
    return result;
}
Point source_point(int family, bool line, double parameter, double gap) {
    if (family == 0) {
        return line ? Point{2 * parameter, .5, 0}
                    : Point{2 * parameter, 4 * parameter * (1 - parameter), gap};
    }
    const double q = std::sqrt(.5);
    if (line) return {4 * parameter - 2, q, 0};
    const double a = (1 - parameter) * (1 - parameter);
    const double b = 2 * q * parameter * (1 - parameter);
    const double c = parameter * parameter;
    return {(a + b) / (a + b + c), (b + c) / (a + b + c), gap};
}
double distance(const Point &a, const Point &b) {
    return std::hypot(std::hypot(a[0] - b[0], a[1] - b[1]), a[2] - b[2]);
}
void array(const std::vector<double> &values) {
    std::cout << '[';
    for (size_t i = 0; i < values.size(); ++i) {
        if (i) std::cout << ',';
        std::cout << values[i];
    }
    std::cout << ']';
}
void print(const Curve &curve) {
    std::cout << "{\"degree\":" << curve.degree << ",\"points_xyzw\":";
    array(curve.points);
    std::cout << ",\"knots\":";
    array(curve.knots);
    std::cout << '}';
}
}

int main() {
    try {
        std::cout << std::setprecision(17) << std::boolalpha;
        size_t cases = 0, failures = 0;
        for (int family : {0, 1})
        for (double gap_factor : {-1.5, -.5, -.05, 0., .05, .5, 1.5})
        for (double scale : {.001, 1., 1000.})
        for (int axis : {0, 1, 2})
        for (bool reversed : {false, true}) {
            ++cases;
            const double tolerance = 1e-8 * scale;
            const double gap = gap_factor * 1e-8;
            const double q = std::sqrt(.5);
            const auto first = family == 0
                ? definition(2, {{0, 0, gap}, {1, 2, gap}, {2, 0, gap}}, {1, 1, 1}, scale, axis)
                : definition(2, {{1, 0, gap}, {1, 1, gap}, {0, 1, gap}}, {1, q, 1}, scale, axis);
            const auto second = family == 0
                ? definition(1, {{0, .5, 0}, {2, .5, 0}}, {1, 1}, scale, axis)
                : definition(1, {{-2, q, 0}, {2, q, 0}}, {1, 1}, scale, axis);
            const auto before_first = first, before_second = second;
            VbCurveIntersections *raw = nullptr;
            char error[2048] = {};
            int status;
            {
                const auto create = [&](const Curve &c) {
                    VbCurve *p = nullptr;
                    if (vb_curve_create(c.degree, c.points.data(), c.points.size() / 4,
                                        c.knots.data(), c.knots.size(), &p, error, sizeof error))
                        throw std::runtime_error(error);
                    return std::unique_ptr<VbCurve, decltype(&vb_curve_free)>(p, &vb_curve_free);
                };
                auto a = create(first), b = create(second);
                status = vb_curve_intersect(reversed ? b.get() : a.get(), reversed ? a.get() : b.get(),
                                            tolerance, &raw, error, sizeof error);
            } // The event buffer must survive both native input curves.
            std::unique_ptr<VbCurveIntersections, decltype(&vb_curve_intersections_free)>
                owner(raw, &vb_curve_intersections_free);
            std::vector<VbCurveIntersection> events(vb_curve_intersections_count(raw));
            if (!events.empty() && vb_curve_intersections_copy(raw, events.data(), events.size()))
                throw std::runtime_error("copy curve contact parameters");
            owner.reset(); // The copied parameters must survive their native buffer too.
            std::sort(events.begin(), events.end(), [&](const auto &a, const auto &b) {
                return (reversed ? a.second[0] : a.first[0]) < (reversed ? b.second[0] : b.first[0]);
            });
            const size_t expected = std::abs(gap_factor) < 1 ? (family == 0 ? 2 : 1) : 0;
            const bool unchanged = first == before_first && second == before_second;
            bool passed = status == 0 && events.size() == expected && unchanged;
            double maximum_gap = 0, maximum_position_error = 0;
            for (size_t i = 0; i < events.size(); ++i) {
                const auto &event = events[i];
                const double u = reversed ? event.second[0] : event.first[0];
                const double v = reversed ? event.first[0] : event.second[0];
                passed &= event.overlap == 0 && u >= 0 && u <= 1 && v >= 0 && v <= 1;
                const auto a = world(source_point(family, false, u, gap), scale, axis);
                const auto b = world(source_point(family, true, v, gap), scale, axis);
                maximum_gap = std::max(maximum_gap, distance(a, b));
                if (i < expected) {
                    const double eu = family == 0 ? (1 + (i == 0 ? -q : q)) / 2 : .5;
                    const double ev = family == 0 ? eu : (q + 2) / 4;
                    maximum_position_error = std::max({maximum_position_error,
                        distance(a, world(source_point(family, false, eu, gap), scale, axis)),
                        distance(b, world(source_point(family, true, ev, gap), scale, axis))});
                }
            }
            passed &= std::isfinite(maximum_gap) && maximum_gap <= tolerance
                && std::isfinite(maximum_position_error) && maximum_position_error <= tolerance;
            failures += !passed;
            std::cout << "{\"family\":" << family << ",\"gap_factor\":" << gap_factor
                      << ",\"scale\":" << scale << ",\"axis\":" << axis
                      << ",\"reversed\":" << reversed << ",\"tolerance\":" << tolerance
                      << ",\"status\":" << status << ",\"expected\":" << expected
                      << ",\"source_unchanged\":" << unchanged << ",\"sources\":[";
            print(first); std::cout << ','; print(second);
            std::cout << "],\"parameters\":[";
            for (size_t i = 0; i < events.size(); ++i) {
                if (i) std::cout << ',';
                std::cout << '[' << events[i].first[0] << ',' << events[i].second[0] << ']';
            }
            std::cout << "],\"maximum_gap\":" << maximum_gap
                      << ",\"maximum_position_error\":" << maximum_position_error
                      << ",\"passed\":" << passed << "}\n";
        }
        std::cerr << cases << " curve contact cases, " << failures << " failures\n";
        return failures ? 1 : 0;
    } catch (const std::exception &e) {
        std::cerr << e.what() << '\n';
        return 2;
    }
}
