// Actual self-intersection ABI, with independent analytic contacts and ownership checks.
#include <bridge.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <limits>
#include <vector>

int main() {
    const std::vector<std::vector<std::array<double,2>>> inputs={
        {{-1,0},{3,2},{-3,2},{1,0}},
        {{0,0},{1,2},{-1,2},{0,0}},
        {{0,0},{2,2},{0,2},{2,0}},
        {{0,0},{2,0},{1,0},{1,2}},
        {{0,0},{2,0},{2,2},{0,2},{0,0}},
        {{0,0},{2,0}}
    };
    bool passed=true,comma=false;
    std::cout<<std::setprecision(17)<<"{\"cases\":[";
    for(size_t kind=0;kind<inputs.size();++kind) for(int chart=0;chart<2;++chart) for(int plane=0;plane<3;++plane) {
        const size_t degree=kind<2?3:1;
        const auto &xy=inputs[kind];
        const double lo=chart?-4:0,hi=chart?7:1;
        std::vector<double> controls,knots(degree+1,lo);
        for(const auto &p:xy) {
            const auto xyz=plane==1?std::array<double,3>{0,p[0],p[1]}:
                plane==2?std::array<double,3>{p[1],0,p[0]}:std::array<double,3>{p[0],p[1],0};
            controls.insert(controls.end(),{xyz[0],xyz[1],xyz[2],1});
        }
        for(size_t i=1;i<xy.size()-degree;++i)knots.push_back(lo+(hi-lo)*i/(xy.size()-degree));
        knots.insert(knots.end(),degree+1,hi);
        VbCurve *curve=nullptr;VbCurveIntersections *events=nullptr;char message[1024]={};
        int status=vb_curve_create(degree,controls.data(),xy.size(),knots.data(),knots.size(),&curve,message,sizeof(message));
        if(!status)status=vb_curve_self_intersect(curve,1e-9,&events,message,sizeof(message));
        std::vector<VbCurveIntersection> records(vb_curve_intersections_count(events));
        if(!status)status=vb_curve_intersections_copy(events,records.data(),records.size());
        if(!status && vb_curve_intersections_copy(events,records.data(),records.size()+1)==0)status=2;
        if(comma)std::cout<<',';comma=true;
        std::cout<<"{\"kind\":"<<kind<<",\"chart\":"<<chart<<",\"plane\":"<<plane<<",\"status\":"<<status<<",\"events\":[";
        size_t accepted=0;double residual=0,parameter_error=0;
        for(size_t i=0;i<records.size();++i) {
            const auto &r=records[i];if(i)std::cout<<',';
            std::cout<<"{\"overlap\":"<<r.overlap<<",\"first\":["<<r.first[0]<<','<<r.first[1]
                <<"],\"second\":["<<r.second[0]<<','<<r.second[1]<<"]}";
            if(!r.overlap && (r.first[0]==r.second[0] ||
                ((kind==1||kind==4) && ((r.first[0]==lo && r.second[0]==hi)||(r.first[0]==hi && r.second[0]==lo)))))continue;
            ++accepted;
            for(int end=0;end<(r.overlap?2:1);++end) {
                double a[3]={},b[3]={};
                if(vb_curve_evaluate(curve,r.first[end],a,message,sizeof(message)) ||
                    vb_curve_evaluate(curve,r.second[end],b,message,sizeof(message))) {status=2;continue;}
                const double gap=std::hypot(std::hypot(a[0]-b[0],a[1]-b[1]),a[2]-b[2]);
                if(!std::isfinite(gap)){status=2;continue;}residual=std::max(residual,gap);
                const double expected_a=kind==0?(1-std::sqrt(0.6))*0.5:kind==2?1./6:(end?1./3:1./6);
                const double expected_b=kind==0?(1+std::sqrt(0.6))*0.5:kind==2?5./6:(end?1./3:2./3);
                const double u=(r.first[end]-lo)/(hi-lo),v=(r.second[end]-lo)/(hi-lo);
                parameter_error=std::max({parameter_error,std::abs(u-expected_a),std::abs(v-expected_b)});
                if(r.overlap!=(kind==3?1:0))status=2;
            }
        }
        std::vector<double> copied_controls(controls.size()),copied_knots(knots.size());
        if(vb_curve_copy(curve,copied_controls.data(),xy.size(),copied_knots.data(),knots.size()) ||
            copied_controls!=controls || copied_knots!=knots)status=2;
        for(double invalid_tolerance:{0.,-1.,std::numeric_limits<double>::infinity(),std::numeric_limits<double>::quiet_NaN()}) {
            VbCurveIntersections *invalid_events=events;
            if(vb_curve_self_intersect(curve,invalid_tolerance,&invalid_events,message,sizeof(message))==0 || invalid_events)status=2;
        }
        const size_t expected=kind==0||kind==2||kind==3?1:0;
        const bool valid=!status && accepted==expected && residual<=1e-9 && parameter_error<=1e-8;
        passed=passed&&valid;
        std::cout<<"],\"accepted_count\":"<<accepted<<",\"residual\":"<<residual<<",\"parameter_error\":"<<parameter_error
            <<",\"passed\":"<<(valid?"true":"false")<<'}';
        vb_curve_intersections_free(events);vb_curve_free(curve);
    }
    VbCurveIntersections *events=nullptr;char message[1024]={};
    const bool invalid=vb_curve_self_intersect(nullptr,1e-9,&events,message,sizeof(message))!=0 && !events;
    passed=passed&&invalid;
    std::cout<<"],\"invalid_input_rejected\":"<<(invalid?"true":"false")<<",\"passed\":"<<(passed?"true":"false")<<"}\n";
    return passed?0:1;
}
