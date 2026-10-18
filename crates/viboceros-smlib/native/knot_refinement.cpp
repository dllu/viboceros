#include "knot_refinement.h"
#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <memory>
#include <initializer_list>
#include <stdexcept>
#include <string>
#include <vector>

namespace {
constexpr size_t limit = 1'000'000;
void require(bool valid, const char *message) {
    if (!valid) throw std::runtime_error(message);
}
void check(NL_FLAG status, const char *message) { require(status == NL_NO, message); }

struct BinaryChart {
    int exponent;
    BinaryChart(const double *knots,size_t count,std::initializer_list<double> parameters={})
        : BinaryChart(knots,count,parameters.begin(),parameters.size()) {}
    BinaryChart(const double *knots,size_t count,const double *parameters,size_t parameter_count) {
        double maximum=0;
        for(size_t i=0;i<count;++i) maximum=std::max(maximum,std::abs(knots[i]));
        std::frexp(maximum,&exponent);
        {
            // A valid closest-parameter query can be subnormal beside zero.
            // Do not force a downward scale that loses that exact coordinate
            // (or a source knot); retain a smaller reversible binary scale.
            const auto reversible=[&](double value) {
                return std::ldexp(std::ldexp(value,-exponent),exponent)==value;
            };
            while(exponent>0 &&
                  (!std::all_of(knots,knots+count,reversible) ||
                   (parameter_count && !std::all_of(parameters,parameters+parameter_count,reversible)))) --exponent;
        }
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
        const size_t source_count=source.pol->n+1;
        const auto *basis=source.knt->U;
        std::vector<size_t> cuts={0};
        for(size_t i=source.p+1;i<source_count;) {
            size_t last=i+1;
            while(last<source_count && basis[last]==basis[i]) ++last;
            if(last-i==static_cast<size_t>(source.p+1)) cuts.push_back(i);
            i=last;
        }
        if(cuts.size()>1) {
            // Full-order breaks separate independent control blocks. The
            // native elevation producer rejects them as one curve; reuse it
            // on contiguous pieces under the same homogeneous gauge.
            require(std::all_of(basis,basis+source.p+1,[&](double k){return k==basis[source.p];})
                && std::all_of(basis+source_count,basis+source.knt->m+1,[&](double k){return k==basis[source_count];}),
                "Piecewise degree elevation requires clamped active endpoints");
            cuts.push_back(source_count);
            std::vector<NL_CPOINT> controls;
            std::vector<double> elevated_knots;
            controls.reserve(knot_count-target-1);
            elevated_knots.reserve(knot_count);
            for(size_t block=0;block+1<cuts.size();++block) {
                const size_t first=cuts[block],count=cuts[block+1]-first;
                require(count>static_cast<size_t>(source.p),"Invalid full-order control block");
                NL_CURVE piece,elevated;
                N_CrvInitArrays(&piece);N_CrvInitArrays(&elevated);
                check(N_CrvFromCPtsAndKnots(&piece,source.pol->Pw+first,count-1,source.p,
                    const_cast<double *>(basis+first),count+source.p,&stack),
                    "Build separated native elevation piece failed");
                check(N_CrvElevateDegree(&piece,target-source.p,&elevated,&stack,&stack),
                    "Elevate separated native curve degree failed");
                require(elevated.p==static_cast<NL_DEGREE>(target) && elevated.pol->n>=0,
                    "Invalid separated elevation output");
                controls.insert(controls.end(),elevated.pol->Pw,elevated.pol->Pw+elevated.pol->n+1);
                const auto *start=elevated.knt->U+(block?target+1:0);
                const double *end=elevated.knt->U+elevated.knt->m+1;
                elevated_knots.insert(elevated_knots.end(),start,end);
            }
            require(elevated_knots.size()==knot_count && controls.size()==knot_count-target-1
                && std::equal(elevated_knots.begin(),elevated_knots.end(),knots),
                "Separated elevation changed the target basis");
            NL_CURVE assembled;
            N_CrvInitArrays(&assembled);
            check(N_CrvFromCPtsAndKnots(&assembled,controls.data(),controls.size()-1,target,
                elevated_knots.data(),elevated_knots.size()-1,&stack),
                "Assemble separated native degree elevation failed");
            // The constructor borrows the vector payload; the producer result
            // must survive these vectors until the caller snapshots its data.
            check(N_CrvCopy(&assembled,&result,&stack),"Copy separated native degree elevation failed");
            return;
        }
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
                        const std::array<double,3> &origin={0,0,0},
                        const double *exact_first=nullptr,const double *exact_last=nullptr);

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
                        const std::array<double,3> &origin,const double *exact_first,
                        const double *exact_last) {
    std::vector<double> euclidean;
    euclidean.reserve(points.size()*4);
    for (size_t i=0;i<points.size();++i) {
        // Controls copied unchanged by extraction must retain their exact
        // Euclidean payload. Projecting a repeated pole through independently
        // conditioned rows can otherwise turn it into a tiny open boundary.
        const double *exact=i==0?exact_first:(i+1==points.size()?exact_last:nullptr);
        if(exact) {
            euclidean.insert(euclidean.end(),exact,exact+4);
            continue;
        }
        const auto &p=points[i];
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
    const BinaryChart chart(knots,data.knot_count[axis],{parameter});
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
    const auto start_group=std::equal_range(full,full+data.knot_count[0],start);
    const auto end_group=std::equal_range(full,full+data.knot_count[0],end);
    const bool isolated_start=start_group.second-start_group.first==static_cast<ptrdiff_t>(degree+1);
    const bool isolated_end=end_group.second-end_group.first==static_cast<ptrdiff_t>(degree+1);
    if(isolated_start && isolated_end) {
        // Full-order boundaries already isolate independent control blocks.
        // Copy them exactly, retaining arbitrary gauges without an unnecessary
        // Euclidean/homogeneous round trip or native interpolation.
        const size_t first=start_group.first-full;
        const size_t after=end_group.first-full;
        require(after>first+degree && after<=count,"Invalid isolated curve segment");
        VbNurbsData output=data;
        output.count[0]=after-first;output.knot_count[0]=output.count[0]+degree+1;
        output.knots[0]=full+first;output.points_xyzw=data.points_xyzw+4*first;
        return vb_copy_nurbs_payload(output);
    }
    const BinaryChart chart(full,data.knot_count[0],{start,end});
    std::vector<double> basis(full,full+data.knot_count[0]);
    chart.scale(basis);
    std::vector<NL_CPOINT> points;
    std::array<double,3> origin;
    const int exponent=conditioned_controls(data,points,origin);
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_CURVE input,result;
    N_CrvInitArrays(&input);N_CrvInitArrays(&result);
    check(N_CrvFromCPtsAndKnots(&input,points.data(),points.size()-1,degree,
          basis.data(),basis.size()-1,&stack),"Build native trim curve failed");
    check(N_CrvExtractCrvSeg(&input,chart.scaled(start),chart.scaled(end),&result,&stack,&stack),
          "Extract native curve segment failed");
    std::vector<double> expected(degree+1,start);
    expected.insert(expected.end(),start_group.second,end_group.first);
    expected.insert(expected.end(),degree+1,end);
    require(result.p==static_cast<NL_DEGREE>(degree) && result.knt->m+1==static_cast<NL_INDEX>(expected.size())
            && result.pol->n+1==static_cast<NL_INDEX>(expected.size()-degree-1),"Native trim layout differs");
    for(size_t i=0;i<expected.size();++i)
        require(result.knt->U[i]==chart.scaled(expected[i]),"Native trim knot basis differs");
    VbNurbsData output=data;
    output.count[0]=result.pol->n+1;output.knots[0]=expected.data();output.knot_count[0]=expected.size();
    points.assign(result.pol->Pw,result.pol->Pw+output.count[0]);
    // Full-order bounds isolate an original control, including interior
    // groups and inactive endpoint padding. Preserve its exact payload.
    const double *exact_first=isolated_start?data.points_xyzw+4*(start_group.first-full):nullptr;
    const double *exact_last=isolated_end?data.points_xyzw+4*(end_group.first-full-1):nullptr;
    return snapshot(output,points,exponent,origin,
                    exact_first,exact_last);
}

VbSurfaceData *vb_trim_nurbs_surface_axis(const VbNurbsData &data,int axis,double start,double end) {
    validate(data);
    require(data.degree[1]>0 && (axis==0 || axis==1),"Invalid native surface strip direction");
    const size_t width=data.count[axis],lines=data.count[1-axis];
    VbNurbsData output=data;
    std::vector<double> controls_out,knots_out,row(4*width);
    for(size_t line=0;line<lines;++line) {
        for(size_t i=0;i<width;++i) {
            const size_t source=axis==0?line*width+i:i*lines+line;
            std::copy_n(data.points_xyzw+4*source,4,row.data()+4*i);
        }
        VbNurbsData curve={{data.degree[axis],0},{width,1},
                           {data.knot_count[axis],0},{data.knots[axis],nullptr},row.data()};
        // Rectangular native patches clamp both axes. A directional strip
        // must instead retain the complete unselected source basis.
        std::unique_ptr<VbSurfaceData,decltype(&vb_surface_data_free)> segment(
            vb_trim_nurbs_curve(curve,start,end),vb_surface_data_free);
        VbNurbsData view;
        require(vb_surface_data_view(segment.get(),&view)==0,"Read native strip control curve failed");
        if(line==0) {
            require(view.count[0]<=limit/lines,"Native strip output resource limit");
            output.count[axis]=view.count[0];output.knot_count[axis]=view.knot_count[0];
            knots_out.assign(view.knots[0],view.knots[0]+view.knot_count[0]);
            controls_out.resize(4*output.count[0]*output.count[1]);
        } else {
            require(view.count[0]==output.count[axis] && view.knot_count[0]==knots_out.size()
                    && std::equal(knots_out.begin(),knots_out.end(),view.knots[0]),
                    "Native strip control curves produced inconsistent bases");
        }
        for(size_t i=0;i<view.count[0];++i) {
            const size_t target=axis==0?line*view.count[0]+i:i*lines+line;
            std::copy_n(view.points_xyzw+4*i,4,controls_out.data()+4*target);
        }
    }
    output.knots[axis]=knots_out.data();output.points_xyzw=controls_out.data();
    return vb_copy_nurbs_payload(output);
}

VbSurfaceData *vb_extend_nurbs_curve(const VbNurbsData &data,double start,double end) {
    validate(data);
    require(data.degree[1]==0 && std::isfinite(start) && std::isfinite(end) && start<end,
            "Invalid native curve extension interval");
    const size_t degree=data.degree[0],count=data.count[0];
    const auto *full=data.knots[0];
    const double active_start=full[degree],active_end=full[count];
    require(start<=active_start && end>=active_end && (start<active_start || end>active_end),
            "Native extension must contain and extend the active domain");
    require(count<=limit-2*degree,"Native curve extension resource limit");
    const BinaryChart chart(full,data.knot_count[0],{start,end});
    std::vector<double> basis(full,full+data.knot_count[0]);
    chart.scale(basis);
    std::vector<NL_CPOINT> points;
    std::array<double,3> origin;
    const int exponent=conditioned_controls(data,points,origin);
    const double common_weight=data.points_xyzw[3];
    bool polynomial=true;
    for(size_t i=1;i<count;++i) polynomial=polynomial && data.points_xyzw[4*i+3]==common_weight;
    if(polynomial) for(size_t i=0;i<count;++i) {
        const auto *p=data.points_xyzw+4*i;
        points[i]={p[0]-origin[0],p[1]-origin[1],p[2]-origin[2],1.};
    }
    const bool negative=std::all_of(points.begin(),points.end(),[](const auto &p){return p.w<0.;});
    if(negative) for(auto &p:points) {p.x=-p.x;p.y=-p.y;p.z=-p.z;p.w=-p.w;}
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_CURVE curve;
    N_CrvInitArrays(&curve);
    check(N_CrvFromCPtsAndKnots(&curve,points.data(),points.size()-1,degree,
          basis.data(),basis.size()-1,&stack),"Build native extension curve failed");
    for(bool at_start:{true,false}) {
        const double requested=at_start?start:end,original=at_start?active_start:active_end;
        if(requested==original) continue;
        const size_t width=curve.pol->n+1;
        const auto *padding=at_start?curve.knt->U:curve.knt->U+width;
        if(!std::all_of(padding,padding+degree+1,[&](double k){return k==chart.scaled(original);})) {
            NL_CURVE clamped;
            N_CrvInitArrays(&clamped);
            clamp_curve_endpoint(curve,!at_start,clamped,stack);
            curve=clamped;
        }
        if(polynomial) for(NL_INDEX i=0;i<=curve.pol->n;++i) curve.pol->Pw[i].w=1.;
        const double target=chart.scaled(requested);
        // A single very long extension makes removal of its original endpoint
        // numerically singular. Grow only by the existing end span each time;
        // that span doubles after exact NL_CMAX removal, preserving the same
        // end polynomial without a new extrapolation or fitting algorithm.
        for(size_t step=0; ; ++step) {
            require(step<4096,"Native extension growth did not converge");
            const size_t current_count=curve.pol->n+1;
            const double current=curve.knt->U[at_start?degree:current_count];
            if(current==target) break;
            const double adjacent=curve.knt->U[at_start?degree+1:current_count-1];
            const double span=std::abs(current-adjacent);
            const double distance=std::abs(target-current);
            // Share the last two stages when needed so the remainder cannot
            // be a tiny sliver relative to the newly enlarged end span.
            const double increment=std::min(span,distance*0.5);
            const double next=distance<=span?target:current+(at_start?-increment:increment);
            require(span>0 && std::isfinite(span) && std::isfinite(next) && next!=current,
                    "Native extension growth lost its active span");
            require(current_count<=limit-degree,"Native extension growth resource limit");
            NL_CURVE extended;
            N_CrvInitArrays(&extended);
            if(N_CrvExtendByParamDist(&curve,next,at_start?NL_START:NL_END,
                  NL_CMAX,&extended,&stack,&stack)!=NL_NO)
                throw std::runtime_error("Extend native curve parameter boundary failed: kernel code "
                    +std::to_string(N_ErrGetType())+", current "+std::to_string(current)
                    +", next "+std::to_string(next)+", controls "+std::to_string(current_count));
            curve=extended;
            // Constant input weights describe a polynomial. Keep its known
            // unit denominator exact so roundoff cannot become a growing
            // rational denominator in subsequent native stages.
            if(polynomial) for(NL_INDEX i=0;i<=curve.pol->n;++i) curve.pol->Pw[i].w=1.;
            if(next!=target) {
                // Keep only the current curve between stages; copying whole
                // large sources into one growing native arena is unbounded.
                points.assign(curve.pol->Pw,curve.pol->Pw+curve.pol->n+1);
                basis.assign(curve.knt->U,curve.knt->U+curve.knt->m+1);
                N_EndNurbs(&stack);
                N_InitNurbs(&stack);
                N_CrvInitArrays(&curve);
                check(N_CrvFromCPtsAndKnots(&curve,points.data(),points.size()-1,degree,
                      basis.data(),basis.size()-1,&stack),"Build native extension growth curve failed");
            }
        }
    }
    const size_t output_count=curve.pol->n+1,knot_count=curve.knt->m+1;
    require(curve.p==static_cast<NL_DEGREE>(degree) && output_count<=limit
        && knot_count==output_count+degree+1,"Native extension output dimensions differ");
    std::vector<double> knots(knot_count);
    for(size_t i=0;i<knot_count;++i) knots[i]=chart.restored(curve.knt->U[i]);
    require(knots[degree]==start && knots[output_count]==end,"Native extension output domain differs");
    points.assign(curve.pol->Pw,curve.pol->Pw+output_count);
    if(negative) for(auto &p:points) {p.x=-p.x;p.y=-p.y;p.z=-p.z;p.w=-p.w;}
    VbNurbsData output=data;
    output.count[0]=output_count;output.knots[0]=knots.data();output.knot_count[0]=knots.size();
    if(polynomial) {
        std::vector<double> euclidean;
        euclidean.reserve(4*output_count);
        for(const auto &p:points) {
            const double values[4]={p.x+origin[0],p.y+origin[1],p.z+origin[2],common_weight};
            for(double value:values) require(std::isfinite(value),"Polynomial extension control exceeds finite range");
            euclidean.insert(euclidean.end(),values,values+4);
        }
        output.points_xyzw=euclidean.data();
        return vb_copy_nurbs_payload(output);
    }
    return snapshot(output,points,exponent,origin);
}

VbSurfaceData *vb_surface_isocurve(const VbNurbsData &data,int varying,double parameter) {
    validate(data);
    require(data.degree[1]>0 && (varying==0 || varying==1) && std::isfinite(parameter),
            "Invalid native isocurve request");
    const int fixed=1-varying;
    const size_t degree=data.degree[fixed],width=data.count[fixed],count=data.count[varying];
    const double *knots=data.knots[fixed];
    const double start=knots[degree],end=knots[width];
    require(parameter>=start && parameter<=end,"Native isocurve parameter outside active domain");
    const size_t multiplicity=std::count(knots,knots+data.knot_count[fixed],parameter);
    VbNurbsData output={{data.degree[varying],0},{count,1},
        {data.knot_count[varying],0},{data.knots[varying],nullptr},nullptr};
    if(multiplicity==degree+1) {
        // Full-order knots isolate a control row exactly. Interior/start
        // queries use the right limit; the active end uses its left limit.
        const size_t first=std::lower_bound(knots,knots+data.knot_count[fixed],parameter)-knots;
        require(parameter!=end || first>0,"Invalid full-order endpoint group");
        const size_t row=parameter==end?first-1:first;
        require(row<width,"Invalid full-order isocurve control row");
        std::vector<double> points(4*count);
        for(size_t i=0;i<count;++i) {
            const size_t source=varying==0?row*count+i:i*width+row;
            std::copy_n(data.points_xyzw+4*source,4,points.data()+4*i);
        }
        output.points_xyzw=points.data();
        return vb_copy_nurbs_payload(output);
    }
    const BinaryChart chart(knots,data.knot_count[fixed],{parameter});
    std::vector<double> basis(knots,knots+data.knot_count[fixed]);
    chart.scale(basis);
    std::vector<NL_CPOINT> points;
    std::array<double,3> origin;
    const int exponent=conditioned_controls(data,points,origin);
    std::vector<NL_CPOINT> transposed(points.size());
    std::vector<NL_CPOINT *> rows(data.count[0]);
    for(size_t u=0;u<data.count[0];++u) {
        rows[u]=transposed.data()+u*data.count[1];
        for(size_t v=0;v<data.count[1];++v) rows[u][v]=points[v*data.count[0]+u];
    }
    VbNurbsData query=data;query.knots[fixed]=basis.data();
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_SURFACE surface;
    NL_CURVE curve;
    N_SrfInitArrays(&surface);N_CrvInitArrays(&curve);
    check(N_SrfFromCPtsAndKnots(&surface,rows.data(),data.count[0]-1,data.count[1]-1,
        data.degree[0],data.degree[1],const_cast<double *>(query.knots[0]),const_cast<double *>(query.knots[1]),
        data.knot_count[0]-1,data.knot_count[1]-1,&stack),"Build native isocurve surface failed");
    check(N_SrfExtractIsoCrv(&surface,chart.scaled(parameter),varying==0?NL_UDIR:NL_VDIR,&curve,&stack),
          "Extract native surface isocurve failed");
    require(curve.p==static_cast<NL_DEGREE>(output.degree[0]) && curve.pol->n+1==static_cast<NL_INDEX>(count)
        && curve.knt->m+1==static_cast<NL_INDEX>(output.knot_count[0])
        && std::equal(output.knots[0],output.knots[0]+output.knot_count[0],curve.knt->U),
        "Native isocurve changed the varying basis");
    points.assign(curve.pol->Pw,curve.pol->Pw+count);
    return snapshot(output,points,exponent,origin);
}

namespace { struct InterpolationBandFailure {}; }

static void solve_interpolation(const VbInterpolationSystem &input,double *output,size_t output_count,bool periodic_band) {
    const bool hermite=input.derivative_orders!=nullptr;
    require(input.knots && input.parameters && input.targets && input.parameter_count>=2
        && input.parameter_count<=(hermite?6:input.point_grid?256:(input.fixed_rows?4096:(input.handles && input.columns==4?10000:1001))) && input.columns>0 && input.columns<=(hermite?16384:2048) && input.columns%4==0
        && input.degree>=1 && input.degree<=NL_MAXDEG
        && (input.closed==0 || input.closed==1) && (input.handles==0 || input.handles==1)
        && (input.uniform_periodic==0 || input.uniform_periodic==1)
        && (!input.handles || (!input.closed && input.degree==3))
        && (!input.uniform_periodic || input.closed)
        && (input.point_grid==0 || input.point_grid==1)
        && (input.nonuniform_periodic==0 || input.nonuniform_periodic==1)
        && (!hermite || (!input.closed && !input.handles && !input.fixed_rows && !input.point_grid
            && input.degree<=5 && input.parameter_count==input.degree+1 && input.derivative_count==input.parameter_count))
        && (hermite || input.derivative_count==0)
        && (!input.nonuniform_periodic || (input.closed && !input.uniform_periodic && !input.handles && !input.fixed_rows && !input.point_grid)),
        "Invalid interpolation dimensions");
    const size_t sections=input.parameter_count-(input.closed && !input.point_grid);
    const size_t unknowns=sections+2*input.handles,controls=unknowns+(input.closed?input.degree:0);
    require(sections>=2 && (!(input.uniform_periodic || input.nonuniform_periodic) || sections>=input.degree) && controls>input.degree && input.knot_count==controls+input.degree+1
        && (input.nonuniform_periodic?input.periodic_control_shift<sections:input.periodic_control_shift==0)
        && (input.fixed_rows?(input.fixed_count==sections && !input.closed && !input.handles):input.fixed_count==0)
        && (input.point_grid?(input.amplification && input.amplification_count==sections && !input.fixed_rows && !input.handles && !input.uniform_periodic):
            (!input.amplification && input.amplification_count==0))
        && input.target_count==unknowns*input.columns && output_count==input.target_count,
        "Invalid interpolation layout");
    std::vector<double> basis(input.knots,input.knots+input.knot_count);
    for(size_t i=0;i<basis.size();++i)
        require(std::isfinite(basis[i]) && (!i || basis[i]>=basis[i-1]),"Invalid interpolation knot vector");
    if(hermite) {
        size_t first=0;
        while(first<input.parameter_count && input.parameters[first]==0.) ++first;
        require(first>=1 && first<=3 && input.parameter_count>first && input.parameter_count-first<=3,
                "Invalid Hermite endpoint constraint counts");
        for(size_t i=0;i<input.parameter_count;++i)
            require(input.parameters[i]==(i<first?0.:1.)
                && input.derivative_orders[i]==(i<first?i:input.parameter_count-1-i),
                "Invalid Hermite endpoint derivative order");
        for(size_t i=0;i<basis.size();++i)
            require(basis[i]==(i<=input.degree?0.:1.),"Hermite basis must be a normalized Bezier interval");
    }
    const BinaryChart chart(input.knots,input.knot_count,input.parameters,input.parameter_count);
    chart.scale(basis);
    for(size_t i=0;i<input.parameter_count;++i)
        require(std::isfinite(input.parameters[i]) && (!i || input.parameters[i]>input.parameters[i-1]
            || (hermite && input.parameters[i]==input.parameters[i-1])
            || (input.nonuniform_periodic && i+1==input.parameter_count && input.parameters[i]==input.parameters[i-1])
            || (input.point_grid && input.closed && input.parameters[i]==input.parameters[i-1])
            || (input.fixed_rows && input.fixed_rows[i] && input.fixed_rows[i-1] && input.parameters[i]==input.parameters[i-1])),
            "Invalid interpolation stations");
    require(input.point_grid || (chart.scaled(input.parameters[0])>=basis[input.degree]
        && chart.scaled(input.parameters[input.parameter_count-1])==basis[controls]
        && (input.uniform_periodic || input.nonuniform_periodic || chart.scaled(input.parameters[0])==basis[input.degree])),
        "Interpolation stations differ from the active interval");

    // Align periodic columns with the actual Greville control phase, then
    // reuse alternate-end cyclic ordering. Exact support is checked before
    // factorization, with a retry through native full pivoted storage.
    const bool cyclic=input.uniform_periodic || periodic_band;
    std::vector<size_t> order(unknowns),inverse(unknowns);
    for(size_t i=0;i<unknowns;++i) {
        order[i]=cyclic?(i%2?(unknowns-1-i/2):i/2):i;
        inverse[order[i]]=i;
    }
    const size_t shift=input.uniform_periodic?input.degree/2:
        (periodic_band && input.nonuniform_periodic?input.periodic_control_shift:0);
    const auto source_row=[&](size_t row) {
        if(cyclic) return order[row];
        if(!input.handles || row==0) return row;
        if(row==1) return sections;
        if(row+2==unknowns) return sections+1;
        if(row+1==unknowns) return sections-1;
        return row-1;
    };
    const auto source_column=[&](size_t column) {
        return cyclic?(order[column]+shift)%sections:column;
    };
    const bool full=input.closed && !cyclic;
    const size_t half_width=std::min(unknowns-1,periodic_band?2*input.degree:input.uniform_periodic?2*(input.degree/2):
        (input.handles || (input.fixed_rows && input.degree==3)?2:input.degree));
    const size_t matrix_width=full?unknowns:2*half_width+1;
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_RMATRIX matrix;
    check(N_SetRealMatrix(&matrix,unknowns-1,unknowns-1,full?NL_MT_FULL:NL_MT_BANDED,
        full?unknowns-1:matrix_width,&stack),"Allocate native interpolation matrix failed");
    NL_REAL **values;
    N_GetRealMatrixPtr(&matrix,&values);
    for(size_t i=0;i<unknowns;++i) std::fill_n(values[i],matrix_width,0.0);
    NL_KNOTVECTOR knots={static_cast<NL_INDEX>(basis.size()-1),basis.data()};
    for(size_t i=0;i<sections;++i) {
        if(input.fixed_rows) {
            require(input.fixed_rows[i]<=1,"Invalid fixed interpolation row");
            if(input.fixed_rows[i]) {values[i][half_width]=1.;continue;}
        }
        double derivatives[3][NL_MAXDEG+1],*rows[]={derivatives[0],derivatives[1],derivatives[2]};
        const size_t order=hermite?input.derivative_orders[i]:0;
        NL_INDEX span;
        const double parameter=chart.scaled(input.parameters[i]);
        // Value rows honor unclamped periodic endpoints; Hermite rows retain
        // the requested one-sided endpoint derivative order.
        check(N_BasisDerivs(&knots,input.degree,parameter,NL_LEFT,order,rows,&span),"Evaluate native interpolation basis failed");
        size_t expected;
        if(input.point_grid && parameter<=basis[input.degree]) {
            expected=input.degree;
            while(expected+1<controls && basis[expected]==basis[expected+1]) ++expected;
        } else if(parameter>=basis[controls]) {
            expected=controls-1;
            while(expected>input.degree && basis[expected]==basis[expected+1]) --expected;
        } else expected=std::upper_bound(basis.begin(),basis.end(),parameter)-basis.begin()-1;
        require(span==static_cast<NL_INDEX>(expected),"Native interpolation span differs from the exact chart");
        const size_t row=cyclic?inverse[i]:
            (input.handles?(i==0?0:(i+1==sections?unknowns-1:i+1)):i);
        if(input.point_grid) input.amplification[i]=0;
        for(size_t j=0;j<=input.degree;++j) {
            const auto original_column=static_cast<size_t>(span-input.degree)+j;
            // Restore derivatives to the caller's parameter after binary chart
            // scaling. Zeroth-order interpolation keeps the existing behavior.
            const double coefficient=order?std::ldexp(derivatives[order][j],-chart.exponent*static_cast<int>(order)):
                derivatives[0][j];
            require(original_column<controls && std::isfinite(coefficient),"Invalid native interpolation basis coefficient");
            if(input.point_grid) input.amplification[i]+=std::abs(coefficient);
            const auto folded=input.closed?original_column%sections:original_column;
            const size_t column=cyclic?inverse[(folded+sections-shift)%sections]:folded;
            if(full) values[row][column]+=coefficient;
            else if(coefficient!=0) {
                const auto band_column=static_cast<ptrdiff_t>(column)-static_cast<ptrdiff_t>(row)+static_cast<ptrdiff_t>(half_width);
                const bool inside=band_column>=0 && static_cast<size_t>(band_column)<matrix_width;
                if(periodic_band && !inside) throw InterpolationBandFailure{};
                require(inside,"Native interpolation exceeded its matrix band");
                values[row][band_column]+=coefficient;
            }
        }
    }
    if(input.handles) {values[1][half_width]=1.;values[unknowns-2][half_width]=1.;}
    std::vector<double> original(unknowns*matrix_width);
    for(size_t i=0;i<unknowns;++i) std::copy_n(values[i],matrix_width,original.data()+i*matrix_width);
    std::vector<NL_INDEX> pivots(full?unknowns:0);
    const auto factor_status=full?N_RealMatrixLuDecomposePivot(&matrix,pivots.data()):N_RealMatrixLuDecompose(&matrix);
    if(periodic_band && factor_status!=NL_NO) throw InterpolationBandFailure{};
    check(factor_status,"Factor native interpolation system failed");
    std::vector<NL_CPOINT> targets(unknowns),solution(unknowns);
    std::vector<double> rhs((input.point_grid || input.nonuniform_periodic) && (full || periodic_band)?unknowns:0),answer(rhs.size());
    for(size_t column=0;column<input.columns;column+=4) {
        for(size_t i=0;i<unknowns;++i) {
            const double *p=input.targets+source_row(i)*input.columns+column;
            require(std::all_of(p,p+4,[](double x){return std::isfinite(x);}),"Nonfinite interpolation targets");
            targets[i]={p[0],p[1],p[2],p[3]};
        }
        // Homogeneous back solving avoids the real-vector RHS tolerance skip.
        for(size_t i=0;full && i<unknowns;++i) {
            require(pivots[i]>=0 && static_cast<size_t>(pivots[i])<unknowns,"Invalid native interpolation pivot");
            std::swap(targets[i],targets[pivots[i]]);
        }
        if((input.point_grid || input.nonuniform_periodic) && (full || periodic_band)) {
            // The native scalar solver has no RHS tolerance skip and avoids
            // out-of-line point blending for every dense LU coefficient.
            for(size_t c=0;c<4;++c) {
                for(size_t i=0;i<unknowns;++i) {
                    const auto &p=targets[i];
                    const double channels[]={p.x,p.y,p.z,p.w};
                    rhs[i]=channels[c];
                }
                const auto solve_status=N_RealMatrixRightForBack(&matrix,rhs.data(),answer.data());
                if(periodic_band && solve_status!=NL_NO) throw InterpolationBandFailure{};
                check(solve_status,"Solve native interpolation scalar channels failed");
                for(size_t i=0;i<unknowns;++i) {
                    auto &p=solution[i];
                    if(c==0) p.x=answer[i];
                    else if(c==1) p.y=answer[i];
                    else if(c==2) p.z=answer[i];
                    else p.w=answer[i];
                }
            }
        } else check(N_RealMatrixForBack(&matrix,targets.data(),NL_HPOINT,solution.data(),&stack),
            "Solve native interpolation homogeneous channels failed");
        for(size_t i=0;i<unknowns;++i) {
            const auto &p=solution[i];
            const double result[]={p.x,p.y,p.z,p.w};
            const bool finite=std::all_of(result,result+4,[](double x){return std::isfinite(x);});
            if(periodic_band && !finite) throw InterpolationBandFailure{};
            require(finite,"Nonfinite interpolation solution");
            std::copy_n(result,4,output+source_column(i)*input.columns+column);
        }
        for(size_t c=0;c<4;++c) {
            double magnitude=0;
            for(size_t i=0;i<unknowns;++i) magnitude=std::max(magnitude,
                std::max(std::abs(input.targets[i*input.columns+column+c]),std::abs(output[i*input.columns+column+c])));
            if(magnitude==0) continue;
            for(size_t row=0;row<unknowns;++row) {
                const double target=input.targets[source_row(row)*input.columns+column+c]/magnitude;
                double sum=0,scale=std::abs(target);
                const size_t first=full?0:(row>half_width?row-half_width:0),last=full?unknowns:std::min(unknowns,row+half_width+1);
                for(size_t i=first;i<last;++i) {
                    const size_t entry=full?i:static_cast<ptrdiff_t>(i)-static_cast<ptrdiff_t>(row)+static_cast<ptrdiff_t>(half_width);
                    const double value=original[row*matrix_width+entry]*(output[source_column(i)*input.columns+column+c]/magnitude);
                    sum+=value;scale+=std::abs(value);
                }
                const bool accepted=std::isfinite(sum) && std::abs(sum-target)<=1e-12*scale+1e-14;
                if(periodic_band && !accepted) throw InterpolationBandFailure{};
                require(accepted,"Native interpolation solution failed the input residual check");
            }
        }
    }
}

void vb_solve_interpolation(const VbInterpolationSystem &input,double *output,size_t output_count) {
    // Preserve full pivoting for small systems, where a band offers no benefit,
    // and retry it after any typed support/factor/solve/residual band failure.
    const bool point_grid=input.point_grid==1 && input.parameter_count<=256
        && input.parameter_count>4*input.degree+1;
    const bool nonuniform=input.nonuniform_periodic==1 && input.parameter_count<=1001
        && input.parameter_count>4*input.degree+2;
    if(input.closed==1 && input.degree>=1 && input.degree<=NL_MAXDEG && (point_grid || nonuniform)) {
        try {
            solve_interpolation(input,output,output_count,true);
            return;
        } catch(const InterpolationBandFailure &) {}
    }
    solve_interpolation(input,output,output_count,false);
}
