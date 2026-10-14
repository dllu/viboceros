// Independent rational Bezier extrusion fixtures for the native blend producer.
// Run one construction per process so a native failure remains observable.
#include <SmApiGeneral.h>
#include <SmBSplineCurve.h>
#include <SmBSplineSurface.h>
#include <SmBrep.h>
#include <SmCurve.h>
#include <SmEdge.h>
#include <SmEdgeuse.h>
#include <SmFace.h>
#include <SmSurface.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>

namespace {
using Vec = std::array<double, 3>;
constexpr std::array<Vec, 3> controls{{{0., 0., 0.}, {1., 0., .8}, {2., 0., .1}}};

void check(SmStatus status, const char *operation) {
    if (status != SM_SUCCESS)
        throw std::runtime_error(std::string(operation) + " status " + std::to_string(status));
}

double length(Vec v) { return std::hypot(std::hypot(v[0], v[1]), v[2]); }
double distance(const SmPoint3d &p, Vec q) {
    return length({p[0] - q[0], p[1] - q[1], p[2] - q[2]});
}

// Quotient rule on three independently expanded Bernstein polynomials.
std::array<Vec, 4> reference_curve(double t, bool rational) {
    const std::array<double, 3> w = rational ? std::array<double, 3>{1., .6, 1.3}
                                           : std::array<double, 3>{1., 1., 1.};
    const std::array<std::array<double, 3>, 3> basis{{
        {(1.-t)*(1.-t), 2.*t*(1.-t), t*t},
        {-2.*(1.-t), 2.-4.*t, 2.*t},
        {2., -4., 2.}}};
    std::array<Vec, 4> a{};
    std::array<double, 3> denominator{};
    for (size_t k = 0; k < 3; ++k) for (size_t i = 0; i < 3; ++i) {
        denominator[k] += basis[k][i] * w[i];
        for (size_t c = 0; c < 3; ++c) a[k][c] += basis[k][i] * w[i] * controls[i][c];
    }
    for (size_t c = 0; c < 3; ++c) {
        a[0][c] /= denominator[0];
        a[1][c] = (a[1][c] - a[0][c] * denominator[1]) / denominator[0];
        a[2][c] = (a[2][c] - 2.*a[1][c]*denominator[1] - a[0][c]*denominator[2]) / denominator[0];
        a[3][c] = (-3.*a[2][c]*denominator[1] - 3.*a[1][c]*denominator[2]) / denominator[0];
    }
    return a;
}

SmFace *patch(SmBrep &model, double y, std::array<double, 2> domain,
              bool rational, bool elevated) {
    SmTArray<SmPoint3d> points;
    SmTArray<double> weights, u, v;
    for (size_t i = 0; i < 3; ++i) for (double cross : {0., 1.}) {
        points.Add(SmPoint3d(controls[i][0], y+cross, controls[i][2]));
        weights.Add(rational ? std::array<double, 3>{1., .6, 1.3}[i] : 1.);
    }
    u.Add(domain[0]); u.Add(domain[1]); v.Add(0.); v.Add(1.);
    SmTArray<ULONG> mu, mv;
    mu.Add(3); mu.Add(3); mv.Add(2); mv.Add(2);
    SmBSplineSurface *surface = nullptr;
    check(SmBSplineSurface::CreateCanonical(*SmApiGetOrCreateContext(), 2, 1, points,
        SM_SF_UNSPECIFIED, mu, mv, u, v, SM_KT_UNSPECIFIED,
        rational ? &weights : nullptr, nullptr, surface), "create curved patch");
    std::unique_ptr<SmBSplineSurface> guard(surface);
    if (elevated) {
        check(surface->DegreeElevate(SM_SP_U, 3), "elevate second boundary");
        check(surface->DegreeElevate(SM_SP_V, 2), "elevate second cross direction");
    }
    SmTArray<double> inserts;
    for (double t : elevated ? std::array<double, 2>{.23, .67}
                             : std::array<double, 2>{.41, .41})
        inserts.Add(domain[0] + t*(domain[1]-domain[0]));
    check(surface->InsertKnots(SM_SP_U, inserts), "insert asymmetric boundary knots");
    SmFace *face = nullptr;
    check(model.CreateFaceFromSurface(surface, surface->GetNaturalUVDomain(), face), "create curved face");
    guard.release(); // The new model owns its surface.
    return face;
}

SmEdgeuse *boundary(SmBrep &model, SmFace *face, double y) {
    SmTArray<SmEdgeuse *> uses;
    model.GetEdgeuses(uses);
    for (ULONG i = 0; i < uses.GetSize(); ++i) {
        auto *use = uses[i];
        if (use->GetFace() != face) continue;
        SmPoint3d midpoint;
        auto *edge = use->GetEdge();
        check(edge->GetCurve()->EvaluatePoint(edge->GetInterval().Evaluate(.5), midpoint), "boundary midpoint");
        if (std::abs(midpoint[1]-y) < 1e-12) return use;
    }
    throw std::runtime_error("missing curved boundary");
}

bool finite(const SmVector3d &v) {
    return std::isfinite(v[0]) && std::isfinite(v[1]) && std::isfinite(v[2]);
}

// Check the raw derivative samples before any surface fitting. The extrusion
// has S(t,y)=(C(t),y), so constant parameter-direction jets are C^(k)(t)*q^k.
// This separates the chain rule repair from the skinning approximation policy.
std::array<double, 3> sample_errors(SmFace *face, SmEdgeuse *a, SmEdgeuse *b,
                                    bool rational, ULONG direction, double takeoff) {
    std::unique_ptr<SmBSplineCurve> rail(a->GetEdge()->CreateTrimmedNURBSCurve(*SmApiGetOrCreateContext()));
    std::unique_ptr<SmBSplineCurve> reference(b->GetEdge()->CreateTrimmedNURBSCurve(*SmApiGetOrCreateContext()));
    if (!rail || !reference) throw std::runtime_error("missing sampling rails");
    auto interval = rail->GetNaturalInterval();
    if (direction == 2) {
        auto reverse = reference->GetNaturalInterval();
        check(reference->ReverseParameterization(reverse, reverse), "reverse sampling reference");
    }
    check(reference->EditParameterization(interval, FALSE), "align sampling reference");
    SmDerivSurfDefinition definition;
    definition.m_eTangencyType = SM_TT_DIR_TO_REFERENCE_CURVE;
    definition.m_dStartTanLength = takeoff;
    definition.m_dEndTanScale = 1.;
    definition.m_pTangentReference = reference.get();
    SmTArray<double> params;
    SmTArray<SmVector3d> positions, first, second, third;
    check(definition.GetSampleValues(*face->GetSurface(), *rail, interval, FALSE, TRUE,
        params, positions, first, &second, &third), "sample boundary jets");
    std::array<double, 3> error{};
    for (ULONG i = 0; i < params.GetSize(); ++i) {
        const double t = (params[i]-interval.GetMin())/interval.GetLength();
        const auto jets = reference_curve(t, rational);
        const auto opposite = reference_curve(direction == 2 ? 1.-t : t, rational)[0];
        const Vec gap{opposite[0]-jets[0][0], 1., opposite[2]-jets[0][2]};
        const double squared = jets[1][0]*jets[1][0]+jets[1][2]*jets[1][2];
        const double q = (gap[0]*jets[1][0]+gap[2]*jets[1][2])/squared;
        Vec tangent{q*jets[1][0], 1., q*jets[1][2]};
        const double scale = takeoff*length(gap)/length(tangent);
        for (auto &c : tangent) c *= scale;
        const double velocity = q*scale;
        error[0] = std::max(error[0], distance(first[i], tangent));
        Vec d2{}, d3{};
        for (size_t c = 0; c < 3; ++c) {
            d2[c] = jets[2][c]*velocity*velocity;
            d3[c] = jets[3][c]*velocity*velocity*velocity;
        }
        error[1] = std::max(error[1], distance(second[i], d2));
        error[2] = std::max(error[2], distance(third[i], d3));
    }
    return error;
}
} // namespace

int main(int argc, char **argv) {
    try {
        if (argc != 5 && argc != 6) throw std::runtime_error("arguments: domain-case blend-flag rational direction [takeoff]");
        const size_t id = std::stoul(argv[1]);
        const ULONG flag = std::stoul(argv[2]), direction = std::stoul(argv[4]);
        const bool rational = std::stoul(argv[3]) != 0;
        const double takeoff = argc == 6 ? std::stod(argv[5]) : .5;
        if (id >= 4 || flag > 2 || direction < 1 || direction > 2)
            throw std::runtime_error("domain-case 0..3, blend-flag 0..2, direction 1..2");
        if (!(std::isfinite(takeoff) && takeoff > 0.)) throw std::runtime_error("takeoff must be finite and positive");
        constexpr double domains[4][2][2] = {
            {{0., 1.}, {0., 1.}}, {{10., 30.}, {10., 30.}},
            {{0., 1.}, {2., 5.}}, {{10., 30.}, {2., 5.}}};
        SmApiCreateContext();
        std::unique_ptr<SmBrep> model(new (*SmApiGetOrCreateContext()) SmBrep());
        model->SetTolerance(1e-10, FALSE);
        auto *first = patch(*model, 0., {domains[id][0][0], domains[id][0][1]}, rational, false);
        auto *second = patch(*model, 2., {domains[id][1][0], domains[id][1][1]}, rational, true);
        auto *a = boundary(*model, first, 1.);
        auto *b = boundary(*model, second, 2.);
        SmDerivSurfDefinition definition;
        definition.m_dStartTanLength = takeoff;
        definition.m_dEndTanScale = 1.;
        SmSurface *raw = nullptr;
        const auto status = SmBrep::CreateEdgeEdgeBlend(*SmApiGetOrCreateContext(), flag, a, b, direction, &definition, raw);
        std::unique_ptr<SmSurface> output(raw);
        std::cout << std::setprecision(17) << "{\"case\":" << id << ",\"blend_flag\":" << flag
                  << ",\"rational\":" << (rational ? "true" : "false")
                  << ",\"direction_flag\":" << direction << ",\"takeoff\":" << takeoff << ",\"status\":" << status;
        if (status != SM_SUCCESS || !output) { std::cout << "}\n"; return 2; }
        const auto domain = output->GetNaturalUVDomain();
        const auto lo = domain.GetMin(), hi = domain.GetMax();
        const double cross_scale = flag == 0 ? hi[0]-lo[0] : hi[1]-lo[1];
        double boundary_error = 0., hermite_error = 0., plane_error = 0.;
        double cross_curvature_error = 0., along_curvature_error = 0., mixed_curvature_error = 0.;
        double cross_derivative_error = 0., minimum_cross_speed = 1e100;
        bool all_finite = true;
        for (int i = 0; i <= 40; ++i) for (int j = 0; j <= 40; ++j) {
            const double t = i/40., s = j/40.;
            const SmPoint2d uv = flag == 0
                ? SmPoint2d(lo[0]+s*(hi[0]-lo[0]), lo[1]+t*(hi[1]-lo[1]))
                : SmPoint2d(lo[0]+t*(hi[0]-lo[0]), lo[1]+s*(hi[1]-lo[1]));
            SmPoint3d point;
            check(output->EvaluatePoint(uv, point), "blend point");
            all_finite = all_finite && finite(point);
            const auto c = reference_curve(t, rational)[0];
            const double target_y = flag == 0 ? 1.+s : flag == 1
                ? 1.+takeoff*s+(3.-3.*takeoff)*s*s+(2.*takeoff-2.)*s*s*s
                : 1.+takeoff*s+(10.-10.*takeoff)*s*s*s+(15.*takeoff-15.)*s*s*s*s+(6.-6.*takeoff)*s*s*s*s*s;
            if (direction == 1)
                hermite_error = std::max(hermite_error, distance(point, {c[0], target_y, c[2]}));
            if (j != 0 && j != 40) continue;
            const auto jets = reference_curve(direction == 2 && j == 40 ? 1.-t : t, rational);
            boundary_error = std::max(boundary_error, distance(point, {jets[0][0], 1.+s, jets[0][2]}));
            if (flag == 0) continue;
            SmVector3d du, dv, duv, duu, dvv;
            check(output->Evaluate2ndDerivatives(uv, i != 40, j != 40, point, du, dv, duv, duu, dvv), "blend jets");
            all_finite = all_finite && finite(du) && finite(dv) && finite(duu) && finite(duv) && finite(dvv);
            const double tangent_length = length(jets[1]);
            const Vec normal{-jets[1][2]/tangent_length, 0., jets[1][0]/tangent_length};
            const auto normal_dot = [&normal](const SmVector3d &v) {
                return normal[0]*v[0] + normal[2]*v[2];
            };
            const double speed = std::hypot(std::hypot(dv[0], dv[1]), dv[2]);
            if (!(speed > 0. && du.LengthSquared() > 0.)) throw std::runtime_error("singular boundary jets");
            minimum_cross_speed = std::min(minimum_cross_speed, speed*cross_scale);
            plane_error = std::max(plane_error, std::max(std::abs(normal_dot(du))/du.Length(), std::abs(normal_dot(dv))/speed));
            cross_derivative_error = std::max(cross_derivative_error,
                length({dv[0]*cross_scale, dv[1]*cross_scale-takeoff, dv[2]*cross_scale}));
            if (flag == 2) {
                // The native producer aims at the opposite rail. With reverse
                // correspondence its takeoff also has a longitudinal component.
                // Compare the complete second fundamental form of (C(t), y),
                // rather than assuming its cross derivative follows the y axis.
                const double normal_second = normal[0]*jets[2][0]+normal[2]*jets[2][2];
                const double qu = (du[0]*jets[1][0]+du[2]*jets[1][2])/(tangent_length*tangent_length);
                const double qv = (dv[0]*jets[1][0]+dv[2]*jets[1][2])/(tangent_length*tangent_length);
                cross_curvature_error = std::max(cross_curvature_error,
                    std::abs(normal_dot(dvv)-normal_second*qv*qv)/(speed*speed));
                along_curvature_error = std::max(along_curvature_error,
                    std::abs(normal_dot(duu)-normal_second*qu*qu)/du.LengthSquared());
                mixed_curvature_error = std::max(mixed_curvature_error,
                    std::abs(normal_dot(duv)-normal_second*qu*qv)/(du.Length()*speed));
            }
        }
        std::cout << ",\"all_finite\":" << (all_finite ? "true" : "false")
                  << ",\"boundary_correspondence_error\":" << boundary_error;
        if (direction == 1) std::cout << ",\"independent_hermite_error\":" << hermite_error;
        if (flag != 0) std::cout << ",\"tangent_plane_error\":" << plane_error
            << ",\"cross_derivative_error\":" << cross_derivative_error
            << ",\"minimum_cross_speed\":" << minimum_cross_speed;
        if (flag == 2) std::cout << ",\"cross_normal_curvature_error\":" << cross_curvature_error
                               << ",\"along_normal_curvature_error\":" << along_curvature_error
                               << ",\"mixed_normal_curvature_error\":" << mixed_curvature_error;
        if (flag == 2) {
            const auto errors = sample_errors(first, a, b, rational, direction, takeoff);
            std::cout << ",\"sample_first_derivative_error\":" << errors[0]
                      << ",\"sample_second_derivative_error\":" << errors[1]
                      << ",\"sample_third_derivative_error\":" << errors[2];
        }
        std::cout << "}\n";
        return all_finite ? 0 : 3;
    } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
