#include "knot_refinement.h"
#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <vector>

namespace {
constexpr size_t limit = 1'000'000;
void require(bool valid, const char *message) {
    if (!valid) throw std::runtime_error(message);
}
void check(NL_FLAG status, const char *message) { require(status == NL_NO, message); }

void reverse_curve(NL_CURVE &source, NL_CURVE &result, NL_STACKS &stack) {
    check(N_CrvReverse(&source, &result, &stack), "Reverse native refinement curve failed");
    // N_CrvReverse only reverses interior knots and assumes clamped padding.
    // Preserve the complete chart by applying the command's exact negated
    // full-vector convention to the native-reversed controls.
    const auto last = source.knt->m;
    for (NL_INDEX i=0; i<=last; ++i) result.knt->U[i] = -source.knt->U[last-i];
}

void reverse_surface(NL_SURFACE &source, NL_FLAG direction, NL_SURFACE &result, NL_STACKS &stack) {
    check(N_SrfReverse(&source, direction, &result, &stack), "Reverse native refinement surface failed");
    const auto *before = direction == NL_UDIR ? source.knu : source.knv;
    auto *after = direction == NL_UDIR ? result.knu : result.knv;
    for (NL_INDEX i=0; i<=before->m; ++i) after->U[i] = -before->U[before->m-i];
}

void degree_curve(NL_CURVE &source, NL_CURVE &result, size_t target, bool deformable,
                  const double *knots, size_t knot_count, NL_STACKS &stack) {
    if (target > static_cast<size_t>(source.p) && !deformable) {
        check(N_CrvElevateDegree(&source, target-source.p, &result, &stack, &stack),
              "Elevate native curve degree failed");
        return;
    }
    const size_t count = knot_count-target-1;
    std::vector<double> parameters(count), basis(knots,knots+knot_count);
    // The native Greville helper sums before dividing and can overflow on
    // finite charts; native point evaluation can also reject those charts.
    // Use one exact binary scale for the private source and target bases,
    // then restore every original target knot around native-fitted controls.
    double maximum=0;
    for (double value:basis) maximum=std::max(maximum,std::abs(value));
    int exponent;
    std::frexp(maximum,&exponent);
    auto scale_basis=[exponent](std::vector<double> &values) {
        for(double &value:values) {
            const double original=value;
            value=std::ldexp(value,-exponent);
            require(std::ldexp(value,exponent)==original,
                    "Native degree-change chart scaling loses knot precision");
        }
    };
    scale_basis(basis);
    std::vector<double> source_basis(source.knt->U,source.knt->U+source.knt->m+1);
    scale_basis(source_basis);
    NL_CURVE query;
    N_CrvInitArrays(&query);
    check(N_CrvFromCPtsAndKnots(&query,source.pol->Pw,source.pol->n,source.p,
          source_basis.data(),source.knt->m,&stack),
          "Build native degree-change query curve failed");
    std::vector<NL_CPOINT> samples(count);
    for (size_t i=0; i<count; ++i) {
        check(N_CrvGetGrevilleAbscissa(basis.data(), basis.size(), target, i, &parameters[i]),
              "Compute native degree-change stations failed");
        require(std::isfinite(parameters[i]), "Native degree-change station exceeds finite range");
        check(N_CrvEvalPt(&query, parameters[i], NL_LEFT, &samples[i]),
              "Evaluate native homogeneous degree-change sample failed");
    }
    NL_KNOTVECTOR target_basis = {static_cast<NL_INDEX>(knot_count-1),basis.data()};
    check(N_FitCrvInterpGivenParams(samples.data(), count-1, NL_HPOINT, parameters.data(),
                                    &target_basis, target, &result, &stack),
          "Interpolate native degree-change curve failed");
    require(static_cast<size_t>(result.knt->m+1)==knot_count &&
              std::equal(basis.begin(),basis.end(),result.knt->U),
            "Native degree-change fitted basis differs");
    std::copy(knots,knots+knot_count,result.knt->U);
    // The command pins endpoints to the supplied clamped controls.
    result.pol->Pw[0]=source.pol->Pw[0];
    result.pol->Pw[count-1]=source.pol->Pw[source.pol->n];
}

void validate(const VbNurbsData &data) {
    require(data.points_xyzw && data.count[0] && data.count[1] &&
            data.count[0] <= limit / data.count[1], "Knot refinement control resource limit");
    for (size_t d = 0; d < 2; ++d) {
        if (d == 1 && data.degree[d] == 0) {
            require(data.count[d] == 1 && data.knot_count[d] == 0, "Invalid curve refinement payload");
            continue;
        }
        require(data.degree[d] > 0 && data.degree[d] <= NL_MAXDEG &&
                data.count[d] > data.degree[d] && data.knots[d] &&
                data.knot_count[d] == data.count[d] + data.degree[d] + 1,
                "Invalid knot refinement dimensions");
        for (size_t i = 0; i < data.knot_count[d]; ++i)
            require(std::isfinite(data.knots[d][i]) && (!i || data.knots[d][i] >= data.knots[d][i-1]),
                    "Invalid refinement knots");
        require(data.knots[d][data.degree[d]] < data.knots[d][data.count[d]],
                "Empty knot refinement active interval");
    }
}

int controls(const VbNurbsData &, std::vector<NL_CPOINT> &);
VbSurfaceData *snapshot(VbNurbsData, const std::vector<NL_CPOINT> &, int);

VbSurfaceData *degree_nurbs(const VbNurbsData &data, int axis, size_t target,
                              bool deformable, const double *target_knots, size_t knot_count) {
    validate(data);
    require(axis>=0 && axis<=1 && data.degree[axis] && target>0 && target<=NL_MAXDEG &&
              target_knots && knot_count>2*target+1 && knot_count<=limit+65,
            "Invalid native degree-change request");
    const size_t new_count=knot_count-target-1;
    require(new_count<=limit/data.count[1-axis], "Native degree-change output resource limit");
    for(size_t i=0;i<knot_count;++i)
        require(std::isfinite(target_knots[i]) && (!i || target_knots[i]>=target_knots[i-1]),
                "Invalid native degree-change target knots");
    const double start=data.knots[axis][data.degree[axis]],end=data.knots[axis][data.count[axis]];
    require(target_knots[target]==start && target_knots[new_count]==end,
            "Native degree-change target domain differs");
    std::vector<NL_CPOINT> input;
    const int exponent=controls(data,input);
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    VbNurbsData output=data;
    output.degree[axis]=target;output.count[axis]=new_count;
    output.knots[axis]=target_knots;output.knot_count[axis]=knot_count;
    std::vector<NL_CPOINT> changed(output.count[0]*output.count[1]);
    const size_t width=data.count[axis],other=data.count[1-axis];
    for(size_t row=0;row<other;++row) {
        std::vector<NL_CPOINT> controls(width);
        for(size_t i=0;i<width;++i) controls[i]=input[axis==0?row*data.count[0]+i:i*data.count[0]+row];
        NL_CURVE curve,result;
        N_CrvInitArrays(&curve);N_CrvInitArrays(&result);
        check(N_CrvFromCPtsAndKnots(&curve,controls.data(),width-1,data.degree[axis],
              const_cast<double *>(data.knots[axis]),data.knot_count[axis]-1,&stack),
              "Build native degree-change control curve failed");
        degree_curve(curve,result,target,deformable,target_knots,knot_count,stack);
        require(static_cast<size_t>(result.pol->n+1)==new_count && result.p==static_cast<NL_DEGREE>(target),
                "Native degree-change output layout differs");
        require(static_cast<size_t>(result.knt->m+1)==knot_count &&
                  std::equal(target_knots,target_knots+knot_count,result.knt->U),
                "Native degree-change output basis differs");
        for(size_t i=0;i<new_count;++i) changed[axis==0?row*new_count+i:i*output.count[0]+row]=result.pol->Pw[i];
    }
    return snapshot(output,changed,exponent);
}

int controls(const VbNurbsData &data, std::vector<NL_CPOINT> &points) {
    const auto count = data.count[0] * data.count[1];
    double maximum = 0;
    for (size_t i = 0; i < count; ++i) {
        const auto *p = data.points_xyzw + 4*i;
        for (size_t d = 0; d < 4; ++d) require(std::isfinite(p[d]), "Nonfinite refinement control");
        require(p[3] != 0, "Zero refinement control weight");
        maximum = std::max(maximum, std::abs(p[3]));
    }
    int exponent;
    std::frexp(maximum, &exponent);
    points.resize(count);
    for (size_t i = 0; i < count; ++i) {
        const auto *p = data.points_xyzw + 4*i;
        const double w = std::ldexp(p[3], -exponent);
        require(w != 0 && std::isfinite(w), "Refinement weight gauge exceeds finite range");
        points[i] = {p[0]*w, p[1]*w, p[2]*w, w};
    }
    return exponent;
}

VbSurfaceData *snapshot(VbNurbsData data, const std::vector<NL_CPOINT> &points, int exponent) {
    std::vector<double> euclidean;
    euclidean.reserve(points.size()*4);
    for (const auto &p : points) {
        require(p.w != 0 && std::isfinite(p.w), "Native refinement has no finite Euclidean control");
        const double values[4] = {p.x/p.w, p.y/p.w, p.z/p.w, std::ldexp(p.w, exponent)};
        for (double value : values) require(std::isfinite(value), "Native refinement control exceeds finite range");
        euclidean.insert(euclidean.end(), values, values + 4);
    }
    data.points_xyzw = euclidean.data();
    return vb_copy_nurbs_payload(data);
}
}

VbSurfaceData *vb_refine_nurbs(const VbNurbsData &data, int axis, double parameter, size_t target) {
    validate(data);
    require(axis >= 0 && axis <= 1 && data.degree[axis] && std::isfinite(parameter) &&
            target && target <= data.degree[axis] + 1, "Invalid knot refinement request");
    const auto *knots = data.knots[axis];
    const auto degree = data.degree[axis], count = data.count[axis];
    const double start = knots[degree], end = knots[count];
    require(parameter >= start && parameter <= end, "Knot refinement parameter outside active interval");
    const size_t current = std::count(knots, knots + data.knot_count[axis], parameter);
    if (current >= target) return vb_copy_nurbs_payload(data);
    require(count <= limit - target && (count + target) <= limit / data.count[1-axis],
            "Inserted knot control resource limit");
    std::vector<NL_CPOINT> points;
    const int exponent = controls(data, points);
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    VbNurbsData output = data;
    if (data.degree[1] == 0) {
        NL_CURVE input, refined;
        N_CrvInitArrays(&input); N_CrvInitArrays(&refined);
        check(N_CrvFromCPtsAndKnots(&input, points.data(), count-1, degree,
              const_cast<double *>(knots), data.knot_count[0]-1, &stack), "Build native refinement curve failed");
        if (parameter == start || parameter == end) {
            NL_CURVE left, right;
            N_CrvInitArrays(&left); N_CrvInitArrays(&right);
            if (parameter == end) {
                // The NL_LEFT span/multiplicity query assumes a clamped end
                // and can corrupt split buffers for an unclamped active end.
                // Reverse with the existing native operation and clamp its start.
                NL_CURVE reversed;
                N_CrvInitArrays(&reversed);
                reverse_curve(input, reversed, stack);
                const double at = reversed.knt->U[reversed.p];
                check(N_CrvSplit(&reversed, at, &left, &right, &stack), "Clamp reversed native endpoint failed");
                reverse_curve(right, refined, stack);
            } else {
                check(N_CrvSplit(&input, parameter, &left, &right, &stack), "Clamp native curve endpoint failed");
                refined = right;
            }
        } else {
            std::vector<double> added(target-current, parameter);
            NL_KNOTVECTOR additions = {static_cast<NL_INDEX>(added.size()-1), added.data()};
            check(N_CrvRefine(&input, &additions, &refined, &stack, &stack), "Refine native curve knots failed");
        }
        NL_INDEX n, m; NL_DEGREE p; NL_CPOINT *vertices; NL_REAL *full;
        N_CrvGetCPtsDegreeAndKnots(&refined, &n, &vertices, &p, &m, &full);
        require(n >= 0 && static_cast<size_t>(n) < limit, "Native curve refinement output limit");
        output.count[0] = n+1; output.knot_count[0] = m+1; output.knots[0] = full;
        points.assign(vertices, vertices+n+1);
    } else {
        std::vector<NL_CPOINT *> rows(data.count[0]);
        std::vector<NL_CPOINT> transposed(points.size());
        for (size_t i = 0; i < data.count[0]; ++i) {
            rows[i] = transposed.data() + i*data.count[1];
            for (size_t j = 0; j < data.count[1]; ++j) rows[i][j] = points[j*data.count[0]+i];
        }
        NL_SURFACE input, refined;
        N_SrfInitArrays(&input); N_SrfInitArrays(&refined);
        check(N_SrfFromCPtsAndKnots(&input, rows.data(), data.count[0]-1, data.count[1]-1,
              data.degree[0], data.degree[1], const_cast<double *>(data.knots[0]),
              const_cast<double *>(data.knots[1]), data.knot_count[0]-1, data.knot_count[1]-1, &stack),
              "Build native refinement surface failed");
        const auto direction = axis == 0 ? NL_UDIR : NL_VDIR;
        if (parameter == start || parameter == end) {
            NL_SURFACE left, right;
            N_SrfInitArrays(&left); N_SrfInitArrays(&right);
            if (parameter == end) {
                NL_SURFACE reversed;
                N_SrfInitArrays(&reversed);
                reverse_surface(input, direction, reversed, stack);
                const double at = axis == 0 ? reversed.knu->U[reversed.p] : reversed.knv->U[reversed.q];
                check(N_SrfSplit(&reversed, at, direction, &left, &right, &stack), "Clamp reversed native surface endpoint failed");
                reverse_surface(right, direction, refined, stack);
            } else {
                check(N_SrfSplit(&input, parameter, direction, &left, &right, &stack), "Clamp native surface endpoint failed");
                refined = right;
            }
        } else {
            std::vector<double> added(target-current, parameter);
            NL_KNOTVECTOR additions = {static_cast<NL_INDEX>(added.size()-1), added.data()};
            check(N_SrfInsertKnots(&input, &additions, direction, &refined, &stack, &stack), "Refine native surface knots failed");
        }
        NL_INDEX n,m,ku,kv; NL_DEGREE pu,pv; NL_CPOINT **vertices; NL_REAL *u,*v;
        N_SrfGetCPtsDegreesAndKnots(&refined,&n,&m,&vertices,&pu,&pv,&ku,&kv,&u,&v);
        require(n >= 0 && m >= 0 && static_cast<size_t>(n+1) <= limit / static_cast<size_t>(m+1),
                "Native surface refinement output limit");
        output.count[0]=n+1; output.count[1]=m+1;
        output.knot_count[0]=ku+1; output.knot_count[1]=kv+1; output.knots[0]=u; output.knots[1]=v;
        points.clear();
        for (NL_INDEX j=0; j<=m; ++j) for (NL_INDEX i=0; i<=n; ++i) points.push_back(vertices[i][j]);
    }
    return snapshot(output, points, exponent);
}

VbSurfaceData *vb_degree_nurbs(const VbNurbsData &source, int axis, size_t degree,
                              bool deformable, const double *knots, size_t knot_count) {
    return degree_nurbs(source,axis,degree,deformable,knots,knot_count);
}
