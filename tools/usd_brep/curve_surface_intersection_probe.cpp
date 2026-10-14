// Actual C ABI diagnostics with independent polynomial curve/patch formulas.
#include <bridge.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <limits>
#include <vector>
std::array<double,3> rotate(std::array<double,3> p,int plane) {
    if(plane==1)return {p[2],p[0],p[1]};
    if(plane==2)return {p[1],p[2],p[0]};
    return p;
}
int main() {
    bool passed=true,comma=false;
    std::cout<<std::setprecision(17)<<"{\"cases\":[";
    for(int kind=0;kind<9;++kind)for(int chart=0;chart<2;++chart)for(int plane=0;plane<3;++plane) {
        const std::array<std::array<double,3>,3> quadratic={{{0,0.5,1},{0.5,0.5,-1},{1,0.5,1}}};
        std::vector<std::array<double,3>> coordinates;
        const auto line=[&](std::array<double,3> a,std::array<double,3> b) {coordinates.push_back(a);coordinates.push_back(b);};
        if(kind==0 || kind>=7)line({0.5,0.5,-1},{0.5,0.5,1});
        else if(kind==1) {for(const auto &p:quadratic)coordinates.push_back(p);}
        else if(kind==2)line({-1,0.5,0},{2,0.5,0});
        else if(kind==3)line({0.25,0.5,0},{0.75,0.5,0});
        else if(kind==4)line({0,0,0},{1,0,0});
        else if(kind==5)line({0,0.5,1},{1,0.5,1});
        else line({2,0.5,-1},{2,0.5,1});
        const size_t degree=coordinates.size()-1;
        const double t0=chart?-4:0,t1=chart?7:1,u0=chart?10:0,u1=chart?16:1,v0=chart?-8:0,v1=chart?3:1;
        std::vector<double> curve_points,surface_points,curve_knots(degree+1,t0);
        for(auto p:coordinates) {p=rotate(p,plane);curve_points.insert(curve_points.end(),{p[0],p[1],p[2],1});}
        curve_knots.insert(curve_knots.end(),degree+1,t1);
        for(int v=0;v<2;++v)for(int u=0;u<2;++u) {
            const auto p=rotate({double(u),double(v),kind>=7?double(u*v):0},plane);
            surface_points.insert(surface_points.end(),{p[0],p[1],p[2],kind==8 && u==1?2.:1.});
        }
        const double ku[]={u0,u0,u1,u1},kv[]={v0,v0,v1,v1};
        const VbNurbsData surface={{1,1},{2,2},{4,4},{ku,kv},surface_points.data()};
        const auto original_surface=surface_points;
        VbCurve *curve=nullptr;VbCurveSurfaceIntersections *events=nullptr;char error[1024]={};
        int status=vb_curve_create(degree,curve_points.data(),coordinates.size(),curve_knots.data(),curve_knots.size(),&curve,error,sizeof(error));
        if(!status)status=vb_curve_surface_intersect(curve,&surface,1e-9,&events,error,sizeof(error));
        std::vector<VbCurveSurfaceIntersection> records(vb_curve_surface_intersections_count(events));
        if(!status)status=vb_curve_surface_intersections_copy(events,records.data(),records.size());
        if(!status && vb_curve_surface_intersections_copy(events,records.data(),records.size()+1)==0)status=2;
        double source_error=0;
        for(double t:{0.,0.25,0.5,0.75,1.}) {
            std::array<double,3> expected{};
            if(kind==1)expected={t,0.5,(2*t-1)*(2*t-1)};
            else for(int k=0;k<3;++k)expected[k]=coordinates[0][k]*(1-t)+coordinates[1][k]*t;
            expected=rotate(expected,plane);double point[3]={};
            if(vb_curve_evaluate(curve,t0+(t1-t0)*t,point,error,sizeof(error)))status=2;
            const double gap=std::hypot(std::hypot(point[0]-expected[0],point[1]-expected[1]),point[2]-expected[2]);
            if(!std::isfinite(gap))status=2;else source_error=std::max(source_error,gap);
        }
        if(surface_points!=original_surface)status=2;
        for(double tolerance:{0.,-1.,std::numeric_limits<double>::infinity(),std::numeric_limits<double>::quiet_NaN()}) {
            auto *invalid=events;
            if(vb_curve_surface_intersect(curve,&surface,tolerance,&invalid,error,sizeof(error))==0 || invalid)status=2;
        }
        for(size_t invalid_degree:{size_t(0),size_t(18)}) {
            auto malformed=surface;malformed.degree[0]=invalid_degree;
            auto *invalid=events;
            if(vb_curve_surface_intersect(curve,&malformed,1e-9,&invalid,error,sizeof(error))==0 || invalid)status=2;
        }
        if(comma)std::cout<<',';comma=true;
        std::cout<<"{\"kind\":"<<kind<<",\"chart\":"<<chart<<",\"plane\":"<<plane<<",\"status\":"<<status<<",\"events\":[";
        double residual=0,parameter_error=0;
        for(size_t i=0;i<records.size();++i) {
            const auto &r=records[i];if(i)std::cout<<',';
            std::cout<<"{\"overlap\":"<<r.overlap<<",\"curve\":["<<r.curve[0]<<','<<r.curve[1]<<"],\"u\":["<<r.u[0]<<','<<r.u[1]<<"],\"v\":["<<r.v[0]<<','<<r.v[1]<<"]}";
            for(int end=0;end<(r.overlap?2:1);++end) {
                const double t=(r.curve[end]-t0)/(t1-t0),u=(r.u[end]-u0)/(u1-u0),v=(r.v[end]-v0)/(v1-v0);
                std::array<double,3> p{};
                if(kind==1)p={t,0.5,(2*t-1)*(2*t-1)};
                else for(int k=0;k<3;++k)p[k]=coordinates[0][k]*(1-t)+coordinates[1][k]*t;
                const double x=kind==8?2*u/(1+u):u;
                const std::array<double,3> q={x,v,kind>=7?x*v:0};
                const double gap=std::hypot(std::hypot(p[0]-q[0],p[1]-q[1]),p[2]-q[2]);
                if(!std::isfinite(gap))status=2;else residual=std::max(residual,gap);
                const double expected=kind>=7?0.625:kind<2?0.5:kind==2?(end?2./3.:1./3.):double(end);
                const double expected_u=kind>=7?(kind==8?1./3.:0.5):kind<2?0.5:kind==2?double(end):kind==3?(end?0.75:0.25):double(end);
                const double expected_v=kind==4?0:0.5;
                parameter_error=std::max({parameter_error,std::abs(t-expected),std::abs(u-expected_u),std::abs(v-expected_v)});
                if(r.overlap!=(kind>=2 && kind<=4?1:0))status=2;
            }
        }
        const bool valid=!status && records.size()==size_t(kind==5||kind==6?0:1) && residual<=1e-9 && parameter_error<=1e-7 && source_error<=1e-12;
        passed=passed&&valid;
        std::cout<<"],\"residual\":"<<residual<<",\"source_error\":"<<source_error<<",\"parameter_error\":"<<parameter_error<<",\"passed\":"<<(valid?"true":"false")<<'}';
        vb_curve_surface_intersections_free(events);vb_curve_free(curve);
    }
    VbCurveSurfaceIntersections *invalid=nullptr;char error[1024]={};
    const bool invalid_input=vb_curve_surface_intersect(nullptr,nullptr,1e-9,&invalid,error,sizeof(error))!=0 && !invalid;
    passed=passed && invalid_input;
    std::cout<<"],\"invalid_input_rejected\":"<<(invalid_input?"true":"false")<<",\"passed\":"<<(passed?"true":"false")<<"}\n";return passed?0:1;
}
