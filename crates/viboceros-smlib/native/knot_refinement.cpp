#include "knot_refinement.h"
#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <memory>
#include <stdexcept>
#include <vector>

namespace {
constexpr size_t limit = 1'000'000;
void require(bool valid, const char *message) {
    if (!valid) throw std::runtime_error(message);
}
void check(NL_FLAG status, const char *message) { require(status == NL_NO, message); }

struct BinaryChart {
    int exponent;
    BinaryChart(const double *knots,size_t count) {
        double maximum=0;
        for(size_t i=0;i<count;++i) maximum=std::max(maximum,std::abs(knots[i]));
        std::frexp(maximum,&exponent);
    }
    double scaled(double value) const {
        const double result=std::ldexp(value,-exponent);
        require(std::ldexp(result,exponent)==value,
                "Native NURBS chart scaling loses knot precision");
        return result;
    }
    double restored(double value) const {
        const double result=std::ldexp(value,exponent);
        require(std::isfinite(result),"Native NURBS restored chart exceeds finite range");
        return result;
    }
    void scale(std::vector<double> &values) const {
        for(double &value:values) value=scaled(value);
    }
};

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

void clamp_curve_endpoint(NL_CURVE &input,bool end,NL_CURVE &result,NL_STACKS &stack) {
    NL_CURVE left,right;
    N_CrvInitArrays(&left);N_CrvInitArrays(&right);
    if(end) {
        NL_CURVE reversed;
        N_CrvInitArrays(&reversed);
        reverse_curve(input,reversed,stack);
        check(N_CrvSplit(&reversed,reversed.knt->U[reversed.p],&left,&right,&stack),
              "Clamp reversed native endpoint failed");
        reverse_curve(right,result,stack);
    } else {
        check(N_CrvSplit(&input,input.knt->U[input.p],&left,&right,&stack),
              "Clamp native curve endpoint failed");
        result=right;
    }
}

void edit_curve_basis(NL_CURVE &source, NL_CURVE &result, size_t target, bool exact_elevation,
                  const double *knots, size_t knot_count, NL_STACKS &stack) {
    if (exact_elevation) {
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
    const BinaryChart chart(knots,knot_count);
    chart.scale(basis);
    std::vector<double> source_basis(source.knt->U,source.knt->U+source.knt->m+1);
    chart.scale(source_basis);
    NL_CURVE query;
    N_CrvInitArrays(&query);
    check(N_CrvFromCPtsAndKnots(&query,source.pol->Pw,source.pol->n,source.p,
          source_basis.data(),source.knt->m,&stack),
          "Build native basis-edit query curve failed");
    std::vector<NL_CPOINT> samples(count);
    for (size_t i=0; i<count; ++i) {
        // Repeated endpoint averaging can round outside the domain. These
        // clamped samples and the fitter's endpoint identity rows are exact.
        if(i==0 || i+1==count) {
            parameters[i]=i==0?basis.front():basis.back();
            samples[i]=source.pol->Pw[i==0?0:source.pol->n];
            continue;
        }
        check(N_CrvGetGrevilleAbscissa(basis.data(), basis.size(), target, i, &parameters[i]),
              "Compute native basis-edit stations failed");
        require(std::isfinite(parameters[i]), "Native basis-edit station exceeds finite range");
        check(N_CrvEvalPt(&query, parameters[i], NL_LEFT, &samples[i]),
              "Evaluate native homogeneous basis-edit sample failed");
    }
    NL_KNOTVECTOR target_basis = {static_cast<NL_INDEX>(knot_count-1),basis.data()};
    check(N_FitCrvInterpGivenParams(samples.data(), count-1, NL_HPOINT, parameters.data(),
                                    &target_basis, target, &result, &stack),
          "Interpolate native basis-edit curve failed");
    require(static_cast<size_t>(result.knt->m+1)==knot_count &&
              std::equal(basis.begin(),basis.end(),result.knt->U),
            "Native basis-edit fitted basis differs");
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
VbSurfaceData *snapshot(VbNurbsData, const std::vector<NL_CPOINT> &, int,
                        const std::array<double,3> &origin={0,0,0});

VbSurfaceData *basis_nurbs(const VbNurbsData &data, int axis, size_t target,
                              bool exact_elevation, const double *target_knots, size_t knot_count) {
    validate(data);
    require(axis>=0 && axis<=1 && data.degree[axis] && target>0 && target<=NL_MAXDEG &&
              target_knots && knot_count>2*target+1 && knot_count<=limit+65,
            "Invalid native basis-edit request");
    const size_t new_count=knot_count-target-1;
    require(new_count<=limit/data.count[1-axis], "Native basis-edit output resource limit");
    for(size_t i=0;i<knot_count;++i)
        require(std::isfinite(target_knots[i]) && (!i || target_knots[i]>=target_knots[i-1]),
                "Invalid native basis-edit target knots");
    const double start=data.knots[axis][data.degree[axis]],end=data.knots[axis][data.count[axis]];
    require(target_knots[target]==start && target_knots[new_count]==end,
            "Native basis-edit target domain differs");
    require(std::all_of(data.knots[axis],data.knots[axis]+data.degree[axis]+1,
                       [start](double k){return k==start;}) &&
              std::all_of(data.knots[axis]+data.count[axis],data.knots[axis]+data.knot_count[axis],
                       [end](double k){return k==end;}) &&
              std::all_of(target_knots,target_knots+target+1,[start](double k){return k==start;}) &&
              std::all_of(target_knots+new_count,target_knots+knot_count,[end](double k){return k==end;}),
            "Native basis editing requires clamped source and target knots");
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
              "Build native basis-edit control curve failed");
        edit_curve_basis(curve,result,target,exact_elevation,target_knots,knot_count,stack);
        require(static_cast<size_t>(result.pol->n+1)==new_count && result.p==static_cast<NL_DEGREE>(target),
                "Native basis-edit output layout differs");
        require(static_cast<size_t>(result.knt->m+1)==knot_count &&
                  std::equal(target_knots,target_knots+knot_count,result.knt->U),
                "Native basis-edit output basis differs");
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

int conditioned_controls(const VbNurbsData &data,std::vector<NL_CPOINT> &points,
                          std::array<double,3> &origin) {
    // Exterior controls with support ending at the active start must not
    // choose the coordinate origin: their positions can be arbitrarily far
    // from the complete active image.
    std::array<size_t,2> leading={0,0};
    for(size_t axis=0;axis<2;++axis) if(data.degree[axis])
        leading[axis]=std::upper_bound(data.knots[axis],data.knots[axis]+data.knot_count[axis],
                       data.knots[axis][data.degree[axis]])-data.knots[axis]-data.degree[axis]-1;
    const auto *anchor=data.points_xyzw+4*(leading[1]*data.count[0]+leading[0]);
    origin={anchor[0],anchor[1],anchor[2]};
    for(size_t i=0;i<data.count[0]*data.count[1];++i) for(size_t d=0;d<3;++d) {
        const double coordinate=data.points_xyzw[4*i+d],local=coordinate-origin[d];
        if(!std::isfinite(local) || local+origin[d]!=coordinate) origin[d]=0;
    }
    std::vector<double> local(data.points_xyzw,data.points_xyzw+4*data.count[0]*data.count[1]);
    for(size_t i=0;i<local.size()/4;++i) for(size_t d=0;d<3;++d) local[4*i+d]-=origin[d];
    VbNurbsData query=data;query.points_xyzw=local.data();
    return controls(query,points);
}

VbSurfaceData *snapshot(VbNurbsData data, const std::vector<NL_CPOINT> &points, int exponent,
                        const std::array<double,3> &origin) {
    std::vector<double> euclidean;
    euclidean.reserve(points.size()*4);
    for (const auto &p : points) {
        if(p.w==0 || !std::isfinite(p.w))
            throw VbUnrepresentableNurbsControl("Native refinement has no finite Euclidean control");
        const double values[4] = {p.x/p.w+origin[0], p.y/p.w+origin[1], p.z/p.w+origin[2], std::ldexp(p.w, exponent)};
        for (double value : values) if(!std::isfinite(value))
            throw VbUnrepresentableNurbsControl("Native refinement control exceeds finite range");
        if(values[3]==0) throw VbUnrepresentableNurbsControl("Native refinement control weight underflows");
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
    const bool clamp_start=parameter==start,clamp_end=parameter==end;
    const bool full_clamp=target==degree+1 &&
        ((clamp_start && !std::all_of(knots,knots+degree+1,[start](double k){return k==start;})) ||
         (clamp_end && !std::all_of(knots+count,knots+data.knot_count[axis],[end](double k){return k==end;})));
    if (current >= target && !full_clamp) return vb_copy_nurbs_payload(data);
    if (full_clamp && current == degree+1) {
        // A full-order group shifted into the active interval has inactive
        // controls outside it. N_CrvSplit writes before its right polygon for
        // this case. No interpolation is needed: retain the active suffix or
        // prefix and the corresponding complete knot vector exactly.
        const size_t first = std::lower_bound(knots,knots+data.knot_count[axis],parameter)-knots;
        const size_t removed = clamp_start ? first : count-first;
        require(removed && removed<count-degree,"Invalid inactive endpoint controls");
        VbNurbsData output=data;
        output.count[axis]-=removed;
        output.knot_count[axis]-=removed;
        output.knots[axis]+=clamp_start?removed:0;
        std::vector<double> retained(output.count[0]*output.count[1]*4);
        for(size_t j=0;j<output.count[1];++j) for(size_t i=0;i<output.count[0];++i) {
            const size_t u=i+(axis==0 && clamp_start?removed:0);
            const size_t v=j+(axis==1 && clamp_start?removed:0);
            std::copy_n(data.points_xyzw+4*(v*data.count[0]+u),4,
                        retained.data()+4*(j*output.count[0]+i));
        }
        output.points_xyzw=retained.data();
        return vb_copy_nurbs_payload(output);
    }
    require(count <= limit - target && (count + target) <= limit / data.count[1-axis],
            "Inserted knot control resource limit");
    std::vector<NL_CPOINT> points;
    const int exponent = controls(data, points);
    const BinaryChart chart(knots,data.knot_count[axis]);
    std::vector<double> query_knots(knots,knots+data.knot_count[axis]);
    chart.scale(query_knots);
    VbNurbsData query=data;
    query.knots[axis]=query_knots.data();
    knots=query_knots.data();
    parameter=chart.scaled(parameter);
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    VbNurbsData output = data;
    if (data.degree[1] == 0) {
        NL_CURVE input, refined;
        N_CrvInitArrays(&input); N_CrvInitArrays(&refined);
        check(N_CrvFromCPtsAndKnots(&input, points.data(), count-1, degree,
              const_cast<double *>(knots), data.knot_count[0]-1, &stack), "Build native refinement curve failed");
        if (clamp_start || clamp_end) {
            // End splitting uses reverse/start-clamp/reverse to avoid the
            // upstream unclamped active-end span/multiplicity buffer bug.
            clamp_curve_endpoint(input,clamp_end,refined,stack);
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
              data.degree[0], data.degree[1], const_cast<double *>(query.knots[0]),
              const_cast<double *>(query.knots[1]), data.knot_count[0]-1, data.knot_count[1]-1, &stack),
              "Build native refinement surface failed");
        const auto direction = axis == 0 ? NL_UDIR : NL_VDIR;
        if (clamp_start || clamp_end) {
            NL_SURFACE left, right;
            N_SrfInitArrays(&left); N_SrfInitArrays(&right);
            if (clamp_end) {
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
    std::vector<double> restored(output.knots[axis],output.knots[axis]+output.knot_count[axis]);
    for(double &value:restored) value=chart.restored(value);
    output.knots[axis]=restored.data();
    require(restored[data.degree[axis]]==start && restored[output.count[axis]]==end,
            "Native refinement changed the active interval");
    std::vector<double> expected,actual;
    for(size_t i=0;i<data.knot_count[axis];++i)
        if(data.knots[axis][i]>start && data.knots[axis][i]<end) expected.push_back(data.knots[axis][i]);
    if(!clamp_start && !clamp_end) {
        const double inserted=chart.restored(parameter);
        expected.insert(std::upper_bound(expected.begin(),expected.end(),inserted),target-current,inserted);
    }
    for(double value:restored) if(value>start && value<end) actual.push_back(value);
    require(actual==expected,"Native refinement changed the interior knot basis");
    return snapshot(output, points, exponent);
}

VbSurfaceData *vb_degree_nurbs(const VbNurbsData &source, int axis, size_t degree,
                              bool deformable, const double *knots, size_t knot_count) {
    require(axis>=0 && axis<=1,"Invalid basis-edit direction");
    return basis_nurbs(source,axis,degree,degree>source.degree[axis] && !deformable,knots,knot_count);
}

VbSurfaceData *vb_fit_nurbs(const VbNurbsData &source, int axis,
                          const double *knots, size_t knot_count) {
    require(axis>=0 && axis<=1,"Invalid basis-fitting direction");
    return basis_nurbs(source,axis,source.degree[axis],false,knots,knot_count);
}

namespace {
struct ControlNet {
    std::array<size_t,2> degree,count;
    std::array<std::vector<double>,2> knots;
    std::vector<NL_CPOINT> points;
};

std::vector<ControlNet> decompose_axis(const ControlNet &source,int axis) {
    const size_t degree=source.degree[axis],width=source.count[axis],other=source.count[1-axis];
    const auto &knots=source.knots[axis];
    std::vector<std::array<double,2>> intervals;
    for(size_t i=degree;i<width;++i) if(knots[i]<knots[i+1]) intervals.push_back({knots[i],knots[i+1]});
    require(!intervals.empty() && intervals.size()<=limit/((degree+1)*other),
            "Bezier decomposition output resource limit");
    const BinaryChart chart(knots.data(),knots.size());
    std::vector<double> basis=knots;
    chart.scale(basis);
    const double start=basis[degree],end=basis[width];
    // Full-order endpoint groups can have inactive prefix/suffix controls.
    // Trim those exact entries before calling the native splitter.
    const size_t start_first=std::lower_bound(basis.begin(),basis.end(),start)-basis.begin();
    const size_t start_mult=std::count(basis.begin(),basis.end(),start);
    const size_t end_first=std::lower_bound(basis.begin(),basis.end(),end)-basis.begin();
    const size_t end_mult=std::count(basis.begin(),basis.end(),end);
    const size_t prefix=start_mult==degree+1?start_first:0;
    const size_t suffix=end_mult==degree+1?width-end_first:0;
    require(prefix+suffix<width-degree,"Invalid Bezier active controls");
    const size_t active_width=width-prefix-suffix;
    basis=std::vector<double>(basis.begin()+prefix,basis.end()-suffix);
    std::vector<ControlNet> results(intervals.size());
    for(size_t span=0;span<results.size();++span) {
        auto &net=results[span];
        net.degree=source.degree;net.count=source.count;net.count[axis]=degree+1;
        net.knots=source.knots;
        net.knots[axis].assign(degree+1,intervals[span][0]);
        net.knots[axis].insert(net.knots[axis].end(),degree+1,intervals[span][1]);
        net.points.resize(net.count[0]*net.count[1]);
    }
    for(size_t row=0;row<other;++row) {
        NL_STACKS stack;
        SmStackHandler cleanup(&stack);
        std::vector<NL_CPOINT> controls(active_width);
        for(size_t i=0;i<active_width;++i) {
            const size_t index=i+prefix;
            controls[i]=source.points[axis==0?row*width+index:index*source.count[0]+row];
        }
        NL_CURVE curve;
        N_CrvInitArrays(&curve);
        check(N_CrvFromCPtsAndKnots(&curve,controls.data(),active_width-1,degree,
              basis.data(),basis.size()-1,&stack),"Build native Bezier control curve failed");
        for(bool at_end:{false,true}) {
            const auto *u=curve.knt->U;
            const size_t count=curve.pol->n+1;
            const double endpoint=u[at_end?count:degree];
            const auto *first=u+(at_end?count:0);
            if(std::all_of(first,first+degree+1,[endpoint](double value){return value==endpoint;})) continue;
            NL_CURVE clamped;
            N_CrvInitArrays(&clamped);
            clamp_curve_endpoint(curve,at_end,clamped,stack);
            curve=clamped;
        }
        NL_CURVE **pieces=nullptr;
        NL_INDEX last=-1;
        check(N_CrvDecomposeBez(&curve,&pieces,&last,&stack),"Decompose native Bezier curve failed");
        require(last>=0 && static_cast<size_t>(last+1)==intervals.size(),"Native Bezier span count differs");
        for(size_t span=0;span<results.size();++span) {
            const auto &piece=*pieces[span];
            require(piece.p==static_cast<NL_DEGREE>(degree) && piece.pol->n==static_cast<NL_INDEX>(degree)
                    && piece.knt->m==static_cast<NL_INDEX>(2*degree+1),"Native Bezier layout differs");
            for(size_t i=0;i<2*degree+2;++i)
                require(piece.knt->U[i]==chart.scaled(intervals[span][i<=degree?0:1]),
                        "Native Bezier interval differs");
            auto &net=results[span];
            for(size_t i=0;i<=degree;++i)
                net.points[axis==0?row*(degree+1)+i:i*net.count[0]+row]=piece.pol->Pw[i];
        }
    }
    return results;
}
}

void vb_bezier_nurbs(const VbNurbsData &data,int direction,VbSurfaceData **output,size_t count) {
    validate(data);
    require(direction>=0 && direction<=2 && (data.degree[1] || direction==0),
            "Invalid Bezier decomposition direction");
    const size_t width=direction==1?data.count[0]:data.degree[0]+1;
    const size_t height=direction==0?data.count[1]:data.degree[1]+1;
    require(count && width<=limit/height && count<=limit/(width*height),
            "Bezier decomposition aggregate resource limit");
    using Owner=std::unique_ptr<VbSurfaceData,decltype(&vb_surface_data_free)>;
    if(data.degree[1] && direction!=2) {
        // Independent control lines can have vastly different common gauges.
        // Decompose each through the same curve producer and retain its exact
        // gauge when assembling strips; the unselected axis is never fitted.
        const size_t axis=direction,other=data.count[1-axis];
        std::vector<std::vector<double>> points(count,std::vector<double>(width*height*4));
        std::vector<std::array<double,2>> intervals;
        for(size_t i=data.degree[axis];i<data.count[axis];++i)
            if(data.knots[axis][i]<data.knots[axis][i+1])
                intervals.push_back({data.knots[axis][i],data.knots[axis][i+1]});
        require(intervals.size()==count,"Native strip count differs");
        for(size_t row=0;row<other;++row) {
            std::vector<double> controls(data.count[axis]*4);
            for(size_t i=0;i<data.count[axis];++i)
                std::copy_n(data.points_xyzw+4*(axis==0?row*data.count[0]+i:i*data.count[0]+row),
                            4,controls.data()+4*i);
            VbNurbsData curve={};
            curve.degree[0]=data.degree[axis];curve.count[0]=data.count[axis];curve.count[1]=1;
            curve.knots[0]=data.knots[axis];curve.knot_count[0]=data.knot_count[axis];
            curve.points_xyzw=controls.data();
            std::vector<VbSurfaceData *> raw(count,nullptr);
            std::vector<Owner> owners;owners.reserve(count);
            vb_bezier_nurbs(curve,0,raw.data(),count);
            for(auto *pointer:raw) owners.emplace_back(pointer,&vb_surface_data_free);
            for(size_t span=0;span<count;++span) {
                VbNurbsData view;
                require(vb_surface_data_view(owners[span].get(),&view)==0,"Invalid native Bezier snapshot");
                for(size_t i=0;i<=data.degree[axis];++i)
                    std::copy_n(view.points_xyzw+4*i,4,
                                points[span].data()+4*(axis==0?row*width+i:i*width+row));
            }
        }
        std::vector<Owner> snapshots;snapshots.reserve(count);
        for(size_t span=0;span<count;++span) {
            VbNurbsData view=data;view.count[0]=width;view.count[1]=height;
            std::vector<double> basis(data.degree[axis]+1,intervals[span][0]);
            basis.insert(basis.end(),data.degree[axis]+1,intervals[span][1]);
            view.knots[axis]=basis.data();view.knot_count[axis]=basis.size();
            view.points_xyzw=points[span].data();
            snapshots.emplace_back(vb_copy_nurbs_payload(view),&vb_surface_data_free);
        }
        for(size_t i=0;i<count;++i) output[i]=snapshots[i].release();
        return;
    }
    ControlNet source;
    for(size_t axis=0;axis<2;++axis) {
        source.degree[axis]=data.degree[axis];source.count[axis]=data.count[axis];
        if(data.knot_count[axis]) source.knots[axis].assign(data.knots[axis],data.knots[axis]+data.knot_count[axis]);
    }
    std::array<double,3> origin;
    const int exponent=conditioned_controls(data,source.points,origin);
    auto first=decompose_axis(source,direction==1?1:0);
    std::vector<Owner> snapshots;snapshots.reserve(count);
    auto save=[&](const ControlNet &net) {
        require(snapshots.size()<count,"Native Bezier output count exceeds capacity");
        VbNurbsData view=data;
        for(size_t axis=0;axis<2;++axis) {
            view.count[axis]=net.count[axis];view.knots[axis]=net.knots[axis].data();
            view.knot_count[axis]=net.knots[axis].size();
        }
        snapshots.emplace_back(snapshot(view,net.points,exponent,origin),&vb_surface_data_free);
    };
    // Keep all intermediate controls homogeneous. In particular an intermediate
    // zero weight need not produce a final control at infinity.
    for(const auto &strip:first) {
        if(direction==2) {
            for(const auto &patch:decompose_axis(strip,1)) save(patch);
        } else save(strip);
    }
    require(snapshots.size()==count,"Native Bezier output count differs");
    for(size_t i=0;i<count;++i) output[i]=snapshots[i].release();
}

VbSurfaceData *vb_trim_nurbs_curve(const VbNurbsData &data,double start,double end) {
    validate(data);
    require(data.degree[1]==0 && std::isfinite(start) && std::isfinite(end) && start<end,
            "Invalid native curve trim interval");
    const size_t degree=data.degree[0],count=data.count[0];
    const auto *full=data.knots[0];
    const double active_start=full[degree],active_end=full[count];
    require(start>=active_start && end<=active_end,"Native trim interval outside active domain");
    if(start==active_start && end==active_end) return vb_copy_nurbs_payload(data);
    if(std::count(full,full+data.knot_count[0],start)==static_cast<ptrdiff_t>(degree+1)
       && std::count(full,full+data.knot_count[0],end)==static_cast<ptrdiff_t>(degree+1)) {
        // Full-order boundaries already isolate independent control blocks.
        // Copy them exactly, retaining arbitrary gauges without an unnecessary
        // Euclidean/homogeneous round trip or native interpolation.
        const size_t first=std::lower_bound(full,full+data.knot_count[0],start)-full;
        const size_t after=std::lower_bound(full,full+data.knot_count[0],end)-full;
        require(after>first+degree && after<=count,"Invalid isolated curve segment");
        VbNurbsData output=data;
        output.count[0]=after-first;output.knot_count[0]=output.count[0]+degree+1;
        output.knots[0]=full+first;output.points_xyzw=data.points_xyzw+4*first;
        return vb_copy_nurbs_payload(output);
    }
    const BinaryChart chart(full,data.knot_count[0]);
    std::vector<double> basis(full,full+data.knot_count[0]);
    chart.scale(basis);
    std::vector<NL_CPOINT> points;
    std::array<double,3> origin;
    const int exponent=conditioned_controls(data,points,origin);
    // ExtractCrvSeg assumes a clamped active end. A full-order group can
    // instead have inactive suffix controls; remove those exact entries first.
    if(end==active_end && std::count(full,full+data.knot_count[0],active_end)==static_cast<ptrdiff_t>(degree+1)) {
        const size_t first=std::lower_bound(full,full+data.knot_count[0],active_end)-full;
        const size_t suffix=count-first;
        require(suffix<count-degree,"Invalid native trim inactive suffix");
        points.resize(count-suffix);basis.resize(basis.size()-suffix);
    }
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_CURVE input,result;
    N_CrvInitArrays(&input);N_CrvInitArrays(&result);
    check(N_CrvFromCPtsAndKnots(&input,points.data(),points.size()-1,degree,
          basis.data(),basis.size()-1,&stack),"Build native trim curve failed");
    if(end==active_end) {
        const size_t width=input.pol->n+1;
        const auto *padding=input.knt->U+width;
        const double endpoint=chart.scaled(end);
        if(!std::all_of(padding,padding+degree+1,[endpoint](double knot){return knot==endpoint;})) {
            NL_CURVE clamped;
            N_CrvInitArrays(&clamped);
            clamp_curve_endpoint(input,true,clamped,stack);
            input=clamped;
        }
    }
    check(N_CrvExtractCrvSeg(&input,chart.scaled(start),chart.scaled(end),&result,&stack,&stack),
          "Extract native curve segment failed");
    std::vector<double> expected(degree+1,start);
    for(size_t i=0;i<data.knot_count[0];++i) if(full[i]>start && full[i]<end) expected.push_back(full[i]);
    expected.insert(expected.end(),degree+1,end);
    require(result.p==static_cast<NL_DEGREE>(degree) && result.knt->m+1==static_cast<NL_INDEX>(expected.size())
            && result.pol->n+1==static_cast<NL_INDEX>(expected.size()-degree-1),"Native trim layout differs");
    for(size_t i=0;i<expected.size();++i)
        require(result.knt->U[i]==chart.scaled(expected[i]),"Native trim knot basis differs");
    VbNurbsData output=data;
    output.count[0]=result.pol->n+1;output.knots[0]=expected.data();output.knot_count[0]=expected.size();
    points.assign(result.pol->Pw,result.pol->Pw+output.count[0]);
    return snapshot(output,points,exponent,origin);
}
