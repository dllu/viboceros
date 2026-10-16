#include "network_fit.h"
#include <NL_Nurbsdef.h>
#include <nurbs.h>
#include <SmNurbsCrv.h>
#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <vector>

namespace {
void require(bool valid, const char *message) {
    if (!valid) throw std::runtime_error(message);
}
void check(NL_FLAG value, const char *message) { require(value==NL_NO,message); }
double point_error(NL_CURVE &curve,const std::vector<NL_POINT> &points,
                   const double *parameters,std::vector<double> *feet=nullptr,double *squared=nullptr) {
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_GCPTEMP projection;
    N_CrvProjectionInitArrays(&projection);
    NL_CURVEJET jet;
    N_crvJetInit(&jet,&curve);
    double maximum=0.;
    if(feet) feet->clear();
    if(squared) *squared=0.;
    for(size_t i=0;i<points.size();++i) {
        N_crvJetSetParam(&jet,parameters[i]);
        auto point=points[i];
        require(N_crvJetRelax(&jet,&point,&projection,&stack)==NL_TRUE,
                "Network fit point projection did not converge");
        NL_PARAMETER parameter=jet.param;
        NL_POINT projected;
        // This Newton routine returns its best parameter even when roundoff
        // prevents convergence at the requested tolerance. Treat that value
        // only as a proposal and independently evaluate the actual curve.
        // Any such curve point gives an upper bound on point-to-image distance.
        N_CrvClosestPt(&curve,point,parameter,1e-12,1e-12,&parameter,&projected);
        require(std::isfinite(parameter) && parameter>=0. && parameter<=1.,
              "Network fit projection left the curve domain");
        check(N_CrvEval(&curve,parameter,NL_LEFT,&projected),"Network fit point evaluation failed");
        const double error=std::hypot(std::hypot(projected.x-point.x,projected.y-point.y),projected.z-point.z);
        require(std::isfinite(error),"Nonfinite network fit point error");
        maximum=std::max(maximum,error);
        if(feet) feet->push_back(parameter);
        if(squared) *squared+=error*error;
    }
    return maximum;
}
}

VbSurfaceData *vb_fit_network_curve(const double *xyz,const double *parameters,
    size_t point_count,const size_t *fixed,size_t fixed_count,
    const double *knots,size_t knot_count,const double *end_derivatives,
    double tolerance,double *maximum_error) {
    require(xyz && parameters && fixed && knots && end_derivatives && maximum_error &&
        point_count>=16 && point_count<=2048 && fixed_count>=2 && fixed_count<=32 &&
        knot_count>=11 && knot_count<=132 && std::isfinite(tolerance) && tolerance>0.,
        "Invalid network fit layout");
    const size_t controls=knot_count-4;
    require(controls<=128 && point_count>controls+fixed_count+2,
            "Network fit control/sample budget");
    require(knots[3]==0. && knots[controls]==1. && parameters[0]==0. &&
        parameters[point_count-1]==1.,"Network fit requires unit charts");
    for(size_t i=0;i<knot_count;++i)
        require(std::isfinite(knots[i]) && (i<4 ? knots[i]==0. :
            (i>=controls ? knots[i]==1. : knots[i]>knots[i-1])),
            "Network fit requires clamped simple cubic knots");
    for(size_t i=0;i<point_count;++i)
        require(std::isfinite(parameters[i]) && parameters[i]>=0. && parameters[i]<=1. &&
            (!i || parameters[i]>parameters[i-1]),"Invalid network fit sample order");
    std::vector<NL_POINT> points(point_count);
    std::vector<double> weights(point_count,1.);
    std::vector<NL_INDEX> indices;
    std::vector<double> fixed_parameters;
    for(size_t i=0;i<point_count;++i) {
        require(std::isfinite(xyz[i*3]) && std::isfinite(xyz[i*3+1]) && std::isfinite(xyz[i*3+2]),
                "Nonfinite network fit point");
        points[i]={xyz[i*3],xyz[i*3+1],xyz[i*3+2]};
    }
    require(fixed[0]==0 && fixed[fixed_count-1]==point_count-1,"Network fit endpoint pins missing");
    for(size_t i=0;i<fixed_count;++i) {
        require(fixed[i]<point_count && (!i || fixed[i]>fixed[i-1]),"Invalid network fit pin index");
        weights[fixed[i]]=-1.;
        indices.push_back(static_cast<NL_INDEX>(fixed[i]));
        fixed_parameters.push_back(parameters[fixed[i]]);
    }
    NL_VECTOR derivatives[2];
    for(size_t i=0;i<2;++i) {
        require(std::isfinite(end_derivatives[i*3]) && std::isfinite(end_derivatives[i*3+1]) &&
            std::isfinite(end_derivatives[i*3+2]),"Nonfinite network fit endpoint derivative");
        derivatives[i]={end_derivatives[i*3],end_derivatives[i*3+1],end_derivatives[i*3+2]};
    }
    NL_REAL derivative_weights[2]={-1.,-1.};
    NL_INDEX derivative_indices[2]={0,static_cast<NL_INDEX>(point_count-1)};
    std::vector<double> basis(knots,knots+knot_count);
    std::vector<double> sites(parameters,parameters+point_count);
    NL_KNOTVECTOR knot_vector={static_cast<NL_INDEX>(knot_count-1),basis.data()};
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_CURVE curve;
    N_CrvInitArrays(&curve);
    check(N_FitCrvWeightedLstSqKnots(points.data(),weights.data(),point_count-1,
        derivatives,derivative_weights,derivative_indices,1,NL_EPOINT,
        sites.data(),&knot_vector,controls-1,3,&curve,&stack),
        "Constrained native network least squares failed");
    *maximum_error=point_error(curve,points,parameters);
    // Correct the free sample parameters while preserving all hard position
    // and endpoint-derivative equations inside the existing constrained solve.
    // Each trial retains the common basis. Squared error drives parameter
    // correction; retain the best measured maximum for final admission.
    // Projection failures or reordered feet never admit output.
    NL_CURVE best=curve;
    double best_maximum=*maximum_error;
    for(size_t pass=0;pass<16 && *maximum_error>tolerance;++pass) {
        std::vector<double> feet;
        double squared;
        point_error(curve,points,parameters,&feet,&squared);
        for(size_t i=0;i<fixed_count;++i) feet[fixed[i]]=parameters[fixed[i]];
        bool ordered=true;
        for(size_t i=0;i<feet.size();++i)
            ordered=ordered && std::isfinite(feet[i]) && feet[i]>=0. && feet[i]<=1. && (!i || feet[i]>feet[i-1]);
        if(!ordered) break;
        NL_CURVE trial;
        N_CrvInitArrays(&trial);
        check(N_FitCrvWeightedLstSqKnots(points.data(),weights.data(),point_count-1,
            derivatives,derivative_weights,derivative_indices,1,NL_EPOINT,
            feet.data(),&knot_vector,controls-1,3,&trial,&stack),
            "Constrained network parameter correction failed");
        double next_squared;
        const double next_error=point_error(trial,points,feet.data(),nullptr,&next_squared);
        if(next_squared>=squared) break;
        curve=trial;
        *maximum_error=next_error;
        if(next_error<best_maximum) {best=trial;best_maximum=next_error;}
    }
    curve=best;
    *maximum_error=best_maximum;
    for(size_t pass=0;pass<4 && *maximum_error>tolerance;++pass) {
        // Existing bounded (100-iteration) geometric point refitter. No global
        // model tolerances change, and its cached errors do not admit output.
        const double before=*maximum_error;
        const std::vector<NL_CPOINT> saved(curve.pol->Pw,curve.pol->Pw+controls);
        check(N_FitCrvApproxPts(points.data(),point_count-1,&curve,-1,fixed_count,
            indices.data(),fixed_parameters.data(),3,0.,0.,1e-12,1e-12,nullptr,&stack),
            "Native network geometric point refit failed");
        *maximum_error=point_error(curve,points,parameters);
        if(*maximum_error>=before) {
            std::copy(saved.begin(),saved.end(),curve.pol->Pw);
            *maximum_error=before;
            break;
        }
    }
    require(curve.p==3 && curve.pol && curve.pol->n+1==static_cast<NL_INDEX>(controls) &&
        curve.knt && curve.knt->m+1==static_cast<NL_INDEX>(knot_count) &&
        std::equal(basis.begin(),basis.end(),curve.knt->U),"Network point fit changed its basis");
    std::vector<double> output;
    output.reserve(controls*4);
    for(size_t i=0;i<controls;++i) {
        const auto &p=curve.pol->Pw[i];
        require(p.w==NL_NOW && std::isfinite(p.x) && std::isfinite(p.y) && std::isfinite(p.z),
            "Invalid polynomial network fit output");
        // NLib marks polynomial controls with NL_NOW rather than a rational
        // unit weight. The public Euclidean payload stores the unit gauge.
        output.insert(output.end(),{p.x,p.y,p.z,1.});
    }
    VbNurbsData data{};
    data.degree[0]=3;data.count[0]=controls;data.count[1]=1;
    data.knot_count[0]=knot_count;data.knots[0]=basis.data();data.points_xyzw=output.data();
    return vb_copy_nurbs_payload(data);
}
