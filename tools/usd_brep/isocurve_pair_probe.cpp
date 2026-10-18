#include <SmApiGeneral.h>
#include <SmBSplineCurve.h>
#include <SmBSplineSurface.h>
#include <SmIsoCurve.h>
#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

namespace {
void check(SmStatus status) {
    if (status != SM_SUCCESS) throw std::runtime_error("isocurve pair probe setup failed");
}
std::vector<double> curve_knots(const SmBSplineCurve &curve) {
    SmTArray<double> values;
    check(curve.GetKnotsAll(values));
    return {values.GetDataArray(), values.GetDataArray() + values.GetSize()};
}
std::vector<double> surface_knots(const SmBSplineSurface &surface, bool varying_u) {
    ULONG nu, nv;
    double *u, *v;
    check(surface.GetKnotsPointers(nu, nv, u, v));
    return varying_u ? std::vector<double>(u, u + nu) : std::vector<double>(v, v + nv);
}
void array(const std::vector<double> &values) {
    std::cout << '[';
    for (size_t i = 0; i < values.size(); ++i) std::cout << (i ? "," : "") << values[i];
    std::cout << ']';
}
int limits_probe() {
    struct Case {
        const char *name;
        int degree;
        std::vector<double> knots;
        double parameter;
        std::vector<double> coefficients;
        bool exact;
    };
    // Independent closed-form endpoint basis values and isolated control rows.
    const std::vector<Case> cases = {
        {"quadratic-start", 2, {-2,-1,0,1,2,3,4}, 0, {.5,.5,0,0}, true},
        {"quadratic-end", 2, {-2,-1,0,1,2,3,4}, 2, {0,0,.5,.5}, true},
        {"quadratic-interior", 2, {-2,-1,0,1,2,3,4}, .5, {.125,.75,.125,0}, true},
        {"quadratic-knot", 2, {-2,-1,0,1,2,3,4}, 1, {0,.5,.5,0}, true},
        {"linear-start", 1, {-1,0,1,2,3}, 0, {1,0,0}, true},
        {"linear-end", 1, {-1,0,1,2,3}, 2, {0,0,1}, true},
        {"partial-multiple-start", 2, {-1,0,0,1,2,3,4}, 0, {1,0,0,0}, true},
        {"partial-multiple-end", 2, {0,0,0,1,2,2,3}, 2, {0,0,0,1}, true},
        {"irregular-start", 2, {-2,-1,0,.5,2,3,4}, 0, {1./3.,2./3.,0,0}, false},
        {"irregular-end", 2, {-2,-1,0,.5,2,3,4}, 2, {0,0,.4,.6}, false},
        {"cubic-start", 3, {-3,-2,-1,0,1,2,3,4,5}, 0, {1./6.,4./6.,1./6.,0,0}, false},
        {"cubic-end", 3, {-3,-2,-1,0,1,2,3,4,5}, 2, {0,0,1./6.,4./6.,1./6.}, false},
        {"full-order-start", 2, {0,0,0,.5,.5,.5,1,1,1}, 0, {1,0,0,0,0,0}, true},
        {"full-order-interior", 2, {0,0,0,.5,.5,.5,1,1,1}, .5, {0,0,0,1,0,0}, true},
        {"full-order-end", 2, {0,0,0,.5,.5,.5,1,1,1}, 1, {0,0,0,0,0,1}, true},
        {"inactive-start", 2, {-2,0,0,0,.5,1,2,3}, 0, {0,1,0,0,0}, true},
        {"inactive-end", 2, {-3,-2,-1,-.5,0,0,0,2}, 0, {0,0,0,1,0}, true},
        {"cubic-full-order", 3, {0,0,0,0,.5,.5,.5,.5,1,1,1,1}, .5, {0,0,0,0,1,0,0,0}, true},
    };
    const auto &context = *SmApiGetOrCreateContext();
    size_t failures = 0;
    for (const auto &test : cases) for (int chart = 0; chart < 4; ++chart)
        for (bool varying_u : {true, false}) for (double gauge : {1., -1.}) {
        const double scale = chart == 0 ? std::ldexp(1.,-40) : chart == 2 ? std::ldexp(1.,40) : 1.;
        const double origin = chart == 3 ? 1e12 : 0.;
        auto fixed = test.knots;
        for (auto &knot : fixed) knot = origin + scale * knot;
        const double parameter = origin + scale * test.parameter;
        // A varying chart with a single-ULP last span must survive untouched.
        const std::vector<double> varying = {7,7,7,8,std::nextafter(9.,8.),9,9,9};
        const auto &u = varying_u ? varying : fixed;
        const auto &v = varying_u ? fixed : varying;
        const size_t nf = test.coefficients.size(), nu = varying_u ? 5 : nf, nv = varying_u ? nf : 5;
        std::vector<double> controls, expected(20,0.);
        for (size_t i = 0; i < nu; ++i) for (size_t j = 0; j < nv; ++j) {
            const size_t k = varying_u ? i : j, row = varying_u ? j : i;
            const double w = gauge * (1. + double((k+2*row)%4) / 4.);
            const std::array<double,4> point = {double(2*k+row)*w, double(row*row)*w, double(k*row)*w,w};
            controls.insert(controls.end(),point.begin(),point.end());
            for (size_t d = 0; d < 4; ++d) expected[4*k+d] += test.coefficients[row]*point[d];
        }
        std::unique_ptr<SmBSplineSurface> source(new (context) SmBSplineSurface);
        check(source->SetExpert(varying_u ? 2 : test.degree, varying_u ? test.degree : 2,
            SM_SF_UNSPECIFIED, SM_EK_CLAMPPED, u.size(),u.data(),v.size(),v.data(),
            SM_CP_HOMOGENEOUS_RATIONAL,nv*4,4,controls.data()));
        SmBSplineCurve *raw = nullptr;
        SmCurve *raw_uv = nullptr;
        const auto status = source->CreateIsoParametricCurve(context,
            varying_u ? SM_SP_V : SM_SP_U,parameter,1e-9,raw,nullptr,nullptr,&raw_uv);
        std::unique_ptr<SmBSplineCurve> curve(raw);
        std::unique_ptr<SmCurve> uv(raw_uv);
        NL_STACKS stack;
        SmStackHandler cleanup(&stack);
        NL_CURVE allocated;
        N_CrvInitArrays(&allocated);
        const auto allocated_status = N_SrfExtractIsoCrv(source->GetOrCreateGwNurbPointer(),parameter,
            varying_u ? NL_UDIR : NL_VDIR,&allocated,&stack);
        bool stack_correct = allocated_status==NL_NO && allocated.p==2 && allocated.pol->n==4
            && allocated.knt->m==7;
        if (stack_correct) {
            stack_correct = std::equal(varying.begin(),varying.end(),allocated.knt->U);
            for (size_t k=0;k<5;++k) {
                const auto &p=allocated.pol->Pw[k];
                const double values[4]={p.x,p.y,p.z,p.w};
                for (size_t d=0;d<4;++d) {
                    const double want=expected[4*k+d];
                    stack_correct &= test.exact ? values[d]==want
                        : std::abs(values[d]-want)<=2e-14*std::max(1.,std::abs(want));
                }
            }
        }
        std::vector<double> actual(20), after(controls.size());
        bool correct = status == SM_SUCCESS && curve && uv;
        double residual = 0.;
        if (correct) {
            check(curve->GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL,0,4,4,actual.data()));
            correct = curve->GetDegree()==2 && curve_knots(*curve)==varying && uv->GetDim()==2;
            const auto interval = uv->GetNaturalInterval();
            correct &= interval.GetMin()==7 && interval.GetMax()==9;
            for (size_t d = 0; d < actual.size(); ++d) {
                const double error = std::abs(actual[d]-expected[d]);
                residual = std::max(residual,error);
                correct &= test.exact ? actual[d]==expected[d] : error<=2e-14*std::max(1.,std::abs(expected[d]));
            }
        }
        check(source->GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL,0,nu-1,0,nv-1,nv*4,4,after.data()));
        const bool unchanged = controls==after && surface_knots(*source,true)==u && surface_knots(*source,false)==v;
        source.reset();
        bool independent = bool(curve && uv);
        if (independent) {
            std::vector<double> owned(20);
            check(curve->GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL,0,4,4,owned.data()));
            independent = owned==actual && curve_knots(*curve)==varying;
            for (double t : {7.,8.,9.}) {
                SmPoint3d point;
                independent &= uv->EvaluatePoint(t,point)==SM_SUCCESS
                    && point.x==(varying_u?t:parameter) && point.y==(varying_u?parameter:t) && point.z==0;
            }
        }
        failures += !(correct && stack_correct && unchanged && independent);
        std::cout << "{\"case\":\"" << test.name << "\",\"chart\":" << chart
            << ",\"varying_u\":" << varying_u << ",\"gauge\":" << gauge << ",\"status\":" << status
            << ",\"correct\":" << correct << ",\"source_unchanged\":" << unchanged
            << ",\"stack_status\":" << allocated_status << ",\"stack_correct\":" << stack_correct
            << ",\"independent_ownership\":" << independent << ",\"exact_required\":" << test.exact
            << ",\"residual\":" << residual << ",\"expected\":";
        array(expected);std::cout << ",\"actual\":";array(actual);std::cout << "}\n";
    }
    return failures ? 1 : 0;
}
int discontinuity_probe(bool reject = false) {
    const auto &context = *SmApiGetOrCreateContext();
    const std::vector<double> u = {0, 0, 1, 1};
    const std::vector<double> v = {0, 0, 0, 0.5, 0.5, 0.5, 1, 1, 1};
    std::vector<double> controls;
    for (size_t i = 0; i < 2; ++i) for (size_t j = 0; j < 6; ++j)
        controls.insert(controls.end(), {double(i), double(j), 0., 1.});
    SmBSplineSurface source;
    source.SetContext(&context);
    check(source.SetExpert(1, 2, SM_SF_UNSPECIFIED, SM_EK_CLAMPPED,
        u.size(), u.data(), v.size(), v.data(), SM_CP_HOMOGENEOUS_RATIONAL, 24, 4, controls.data()));
    SmBSplineCurve *raw = nullptr;
    SmCurve *raw_uv = nullptr;
    const auto status = source.CreateIsoParametricCurve(context, SM_SP_V, reject ? 40. : 0.5, 1e-9,
        raw, nullptr, nullptr, &raw_uv);
    std::unique_ptr<SmBSplineCurve> curve(raw);
    std::unique_ptr<SmCurve> uv(raw_uv);
    std::vector<double> after(controls.size());
    check(source.GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, 1, 0, 5, 24, 4, after.data()));
    bool valid = status != SM_SUCCESS ? !curve && !uv : bool(curve && uv);
    if (reject) valid &= status != SM_SUCCESS;
    if (status == SM_SUCCESS && valid) {
        std::vector<double> actual(8);
        check(curve->GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, 1, 4, actual.data()));
        valid = actual == std::vector<double>{0., 3., 0., 1., 1., 3., 0., 1.};
    }
    const bool unchanged = controls == after && surface_knots(source, false) == v
        && surface_knots(source, true) == u;
    std::cout << "{\"status\":" << status << ",\"curve_present\":" << bool(curve)
        << ",\"uv_present\":" << bool(uv) << ",\"valid_outcome\":" << valid
        << ",\"source_unchanged\":" << unchanged << "}\n";
    return valid && unchanged ? 0 : 1;
}
}

int main(int argc, char **argv) {
    try {
        std::cout << std::setprecision(17) << std::boolalpha;
        if (argc == 2 && std::string(argv[1]) == "--discontinuity") return discontinuity_probe();
        if (argc == 2 && std::string(argv[1]) == "--limits") return limits_probe();
        if (argc == 2 && std::string(argv[1]) == "--rejection") return discontinuity_probe(true);
        const auto &context = *SmApiGetOrCreateContext();
        size_t failures = 0;
        for (int chart = 0; chart < 4; ++chart) for (bool narrow : {false, true}) {
            const double scale = chart == 0 ? std::ldexp(1., -40) : chart == 2 ? std::ldexp(1., 40) : 1.;
            const double origin = chart == 3 ? 1e12 : -3. * scale;
            const double middle = narrow ? std::nextafter(origin + scale, origin) : origin + scale / 2.;
            const std::vector<double> knots = {origin, origin, origin, middle, middle,
                                               origin + scale, origin + scale, origin + scale};
            const std::vector<double> fixed = {-3., -3., 5., 5.};
            for (bool varying_u : {true, false}) {
                const size_t nu = varying_u ? 5 : 2, nv = varying_u ? 2 : 5;
                const auto &u = varying_u ? knots : fixed;
                const auto &v = varying_u ? fixed : knots;
                std::vector<double> controls, expected;
                for (size_t i = 0; i < nu; ++i) for (size_t j = 0; j < nv; ++j) {
                    const size_t k = varying_u ? i : j, row = varying_u ? j : i;
                    const double w = std::array<double, 5>{1., 0.5, 1.5, 0.75, 1.25}[k] * (row ? 2. : 1.);
                    controls.insert(controls.end(), {4. * k * w, double(k * k) * w, 8. * row * w, w});
                }
                for (size_t k = 0; k < 5; ++k) {
                    const size_t first = varying_u ? k * nv : k;
                    const size_t second = varying_u ? first + 1 : nv + k;
                    for (size_t d = 0; d < 4; ++d)
                        expected.push_back(0.75 * controls[4 * first + d] + 0.25 * controls[4 * second + d]);
                }
                std::unique_ptr<SmBSplineSurface> source(new (context) SmBSplineSurface);
                check(source->SetExpert(varying_u ? 2 : 1, varying_u ? 1 : 2,
                    SM_SF_UNSPECIFIED, SM_EK_CLAMPPED, u.size(), u.data(), v.size(), v.data(),
                    SM_CP_HOMOGENEOUS_RATIONAL, nv * 4, 4, controls.data()));
                if (surface_knots(*source, varying_u) != knots)
                    throw std::runtime_error("probe source basis was changed");
                SmBSplineCurve *raw = nullptr;
                SmCurve *raw_uv = nullptr;
                const auto status = source->CreateIsoParametricCurve(context,
                    varying_u ? SM_SP_V : SM_SP_U, -1., 1e-9, raw, nullptr, nullptr, &raw_uv);
                std::unique_ptr<SmBSplineCurve> curve(raw);
                std::unique_ptr<SmCurve> uv(raw_uv);
                std::vector<double> actual_knots, actual(20), after(controls.size());
                if (curve) {
                    actual_knots = curve_knots(*curve);
                    check(curve->GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, 4, 4, actual.data()));
                }
                check(source->GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, nu - 1,
                    0, nv - 1, nv * 4, 4, after.data()));
                const bool exact_curve = status == SM_SUCCESS && curve && curve->GetDegree() == 2
                    && actual_knots == knots && actual == expected;
                bool uv_matches = uv && uv->GetDim() == 2;
                if (uv_matches) {
                    const auto interval = uv->GetNaturalInterval();
                    uv_matches = interval.GetMin() == knots[2] && interval.GetMax() == knots[5];
                    for (double t : {knots[2], middle, knots[5]}) {
                        SmPoint3d point;
                        const auto evaluated = uv->EvaluatePoint(t, point);
                        uv_matches &= evaluated == SM_SUCCESS && point.x == (varying_u ? t : -1.)
                            && point.y == (varying_u ? -1. : t) && point.z == 0.;
                    }
                }
                const bool unchanged = controls == after && surface_knots(*source, varying_u) == knots
                    && surface_knots(*source, !varying_u) == fixed;
                source.reset();
                bool independent = bool(curve && uv);
                if (independent) {
                    std::vector<double> owned(20);
                    check(curve->GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, 4, 4, owned.data()));
                    SmPoint3d point;
                    independent = owned == actual && curve_knots(*curve) == actual_knots
                        && uv->EvaluatePoint(middle, point) == SM_SUCCESS
                        && point.x == (varying_u ? middle : -1.) && point.y == (varying_u ? -1. : middle);
                }
                failures += !(exact_curve && uv_matches && unchanged && independent);
                std::cout << "{\"chart\":" << chart << ",\"narrow\":" << narrow
                    << ",\"varying_u\":" << varying_u << ",\"status\":" << status
                    << ",\"exact_curve\":" << exact_curve << ",\"uv_matches\":" << uv_matches
                    << ",\"source_unchanged\":" << unchanged << ",\"independent_ownership\":" << independent
                    << ",\"expected_knots\":";
                array(knots); std::cout << ",\"actual_knots\":"; array(actual_knots);
                std::cout << ",\"expected_homogeneous\":"; array(expected);
                std::cout << ",\"actual_homogeneous\":"; array(actual); std::cout << "}\n";
            }
        }
        return failures ? 1 : 0;
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 2;
    }
}
