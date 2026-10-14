// Actual native intersection ABI: analytic geometry, copies and ownership.
#include <bridge.h>
#include <array>
#include <algorithm>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <vector>

std::array<double,3> rotate(double x,double y,int plane) {
    if(plane==1)return {0,x,y};
    if(plane==2)return {y,0,x};
    return {x,y,0};
}
double distance(const std::array<double,3>& a,const std::array<double,3>& b) {
    return std::hypot(std::hypot(a[0]-b[0],a[1]-b[1]),a[2]-b[2]);
}
std::array<double,3> source(double u,int kind,int plane) {
    return rotate(2*u,kind==1||kind==2?4*u*(1-u):0,plane);
}
std::array<double,3> target(double u,int kind,int plane) {
    if(kind==0)return rotate(1,-1+2*u,plane);
    if(kind==1||kind==2)return rotate(2*u,kind==1?1:0.5,plane);
    if(kind==3)return rotate(0.5+2.5*u,0,plane);
    if(kind==4)return rotate(3-2.5*u,0,plane);
    return rotate(2*u,2e-8,plane);
}
int main() {
    bool passed=true,comma=false;
    std::cout<<std::setprecision(17)<<"{\"cases\":[";
    for(int kind=0;kind<6;++kind) for(int chart=0;chart<2;++chart) for(int plane=0;plane<3;++plane) {
        const bool quadratic=kind==1||kind==2;
        const double lo=chart?-4:0,hi=chart?7:1,other_lo=chart?10:0,other_hi=chart?16:1;
        std::vector<double> a,b,ka(quadratic?3:2,lo),kb(2,other_lo);
        const std::array<std::array<double,2>,3> controls={{{0,0},{1,2},{2,0}}};
        for(int i=0;i<(quadratic?3:2);++i) {
            const auto p=rotate(quadratic?controls[i][0]:2*i,quadratic?controls[i][1]:0,plane);
            a.insert(a.end(),{p[0],p[1],p[2],1});
        }
        for(int i=0;i<2;++i) {
            const auto p=target(i,kind,plane);b.insert(b.end(),{p[0],p[1],p[2],1});
        }
        ka.insert(ka.end(),quadratic?3:2,hi);kb.insert(kb.end(),2,other_hi);
        VbCurve *first=nullptr,*second=nullptr;VbCurveIntersections *events=nullptr;char message[1024]={};
        int status=vb_curve_create(quadratic?2:1,a.data(),a.size()/4,ka.data(),ka.size(),&first,message,sizeof(message));
        if(!status)status=vb_curve_create(1,b.data(),2,kb.data(),kb.size(),&second,message,sizeof(message));
        if(!status)status=vb_curve_intersect(first,second,1e-9,&events,message,sizeof(message));
        const size_t count=vb_curve_intersections_count(events);
        std::vector<VbCurveIntersection> records(count);
        if(!status)status=vb_curve_intersections_copy(events,records.data(),count);
        double residual=0,parameter_error=0,source_error=0;
        const size_t expected_count=kind==2?2:kind==5?0:1;
        if(count!=expected_count)status=2;
        for(size_t i=0;i<count && !status;++i) {
            const auto r=records[i];const bool overlap=kind==3||kind==4;
            if(r.overlap!=(overlap?1:0)) {status=2;break;}
            for(int end=0;end<(overlap?2:1);++end) {
                const double u=(r.first[end]-lo)/(hi-lo),v=(r.second[end]-other_lo)/(other_hi-other_lo);
                const auto p=source(u,kind,plane),q=target(v,kind,plane);
                const double error=distance(p,q);
                if(!std::isfinite(error)) {status=2;break;}
                residual=std::max(residual,error);
                const double expected_u=overlap?(end?1:0.25):kind<2?0.5:
                    (1+(i?1:-1)*std::sqrt(0.5))*0.5;
                const double expected_v=overlap?(kind==3?(end?0.6:0):(end?0.4:1)):expected_u;
                parameter_error=std::max({parameter_error,std::abs(u-expected_u),std::abs(v-expected_v)});
            }
        }
        if(!status) for(double u:{0.,0.25,0.5,0.75,1.}) {
            double p[3];status=vb_curve_evaluate(first,lo+(hi-lo)*u,p,message,sizeof(message));
            if(status)break;
            const double first_error=distance({p[0],p[1],p[2]},source(u,kind,plane));
            if(!std::isfinite(first_error)) {status=2;break;}
            source_error=std::max(source_error,first_error);
            status=vb_curve_evaluate(second,other_lo+(other_hi-other_lo)*u,p,message,sizeof(message));
            if(status)break;
            const double second_error=distance({p[0],p[1],p[2]},target(u,kind,plane));
            if(!std::isfinite(second_error)) {status=2;break;}
            source_error=std::max(source_error,second_error);
        }
        const bool valid=!status && residual<=1e-9 && parameter_error<=1e-8 && source_error<=1e-12;
        passed=passed&&valid;
        if(comma)std::cout<<',';comma=true;
        std::cout<<"{\"kind\":"<<kind<<",\"chart\":"<<chart<<",\"plane\":"<<plane<<",\"status\":"<<status
            <<",\"count\":"<<count<<",\"point_residual\":"<<residual<<",\"parameter_error\":"<<parameter_error
            <<",\"source_error\":"<<source_error<<",\"passed\":"<<(valid?"true":"false")<<'}';
        vb_curve_intersections_free(events);vb_curve_free(first);vb_curve_free(second);
    }
    VbCurveIntersections *events=nullptr;char message[1024]={};
    const bool invalid=vb_curve_intersect(nullptr,nullptr,1e-9,&events,message,sizeof(message))!=0 && !events;
    passed=passed&&invalid;
    std::cout<<"],\"invalid_input_rejected\":"<<(invalid?"true":"false")<<",\"passed\":"<<(passed?"true":"false")<<"}\n";
    return passed?0:1;
}
