// Actual C ABI: coupled XYZ/UV images, transfer ownership, and input rejection.
#include <bridge.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <limits>
#include <vector>
using Point=std::array<double,3>;
Point rotate(Point p,int plane) {
    if(plane==1)return {p[2],p[0],p[1]};
    if(plane==2)return {p[1],p[2],p[0]};
    return p;
}
struct Input {
    size_t degree[2]={1,1},count[2]={2,2};
    std::vector<double> controls,knots[2];
    double lower[2],upper[2];
    std::vector<Point> points;
    VbNurbsData view() const {
        return {{degree[0],degree[1]},{count[0],count[1]},
            {knots[0].size(),knots[1].size()}, {knots[0].data(),knots[1].data()},controls.data()};
    }
    Input(std::array<Point,4> p,int chart,int plane,bool polynomial=false):points(p.begin(),p.end()) {
        lower[0]=chart?10:0;upper[0]=chart?16:1;
        lower[1]=chart?-8:0;upper[1]=chart?3:1;
        if(polynomial) {
            degree[0]=2;degree[1]=3;count[0]=3;count[1]=4;points.clear();
            for(int v=0;v<4;++v)for(int u=0;u<3;++u)
                points.push_back({u/2.,v/3.,double(u==2)+double(v==3)});
        }
        for(auto q:points) {q=rotate(q,plane);controls.insert(controls.end(),{q[0],q[1],q[2],1});}
        for(int axis=0;axis<2;++axis) {
            knots[axis].assign(degree[axis]+1,lower[axis]);
            knots[axis].insert(knots[axis].end(),degree[axis]+1,upper[axis]);
        }
    }
    Point image(double u,double v,int plane) const {
        u=(u-lower[0])/(upper[0]-lower[0]);v=(v-lower[1])/(upper[1]-lower[1]);
        if(degree[0]==2)return rotate({u,v,u*u+v*v*v},plane);
        Point q{};
        for(int k=0;k<3;++k)q[k]=points[0][k]*(1-u)*(1-v)+points[1][k]*u*(1-v)+points[2][k]*(1-u)*v+points[3][k]*u*v;
        return rotate(q,plane);
    }
};
double gap(Point a,Point b) {return std::hypot(std::hypot(a[0]-b[0],a[1]-b[1]),a[2]-b[2]);}
int main() {
    bool passed=true,comma=false;
    std::cout<<std::unitbuf<<std::setprecision(17)<<"{\"cases\":[";
    for(int kind=0;kind<7;++kind)for(int chart=0;chart<2;++chart)for(int plane=0;plane<3;++plane) {
        std::array<Point,4> ap={{{0,0,0},{1,0,0},{0,1,0},{1,1,0}}},bp;
        if(kind==0)bp={{{0.5,0,-1},{0.5,1,-1},{0.5,0,1},{0.5,1,1}}};
        else if(kind==1)bp={{{0,0,1},{1,0,1},{0,1,1},{1,1,1}}};
        else if(kind==2)bp={{{0.25,0.25,0},{1.25,0.25,0},{0.25,1.25,0},{1.25,1.25,0}}};
        else if(kind==3)bp={{{1,1,0},{2,1,0},{1,2,0},{2,2,0}}};
        else if(kind==4)bp={{{0,0,0.25},{1,0,-0.25},{0,1,-0.25},{1,1,0.25}}};
        else if(kind==5) {
            ap={{{0,0,0.25},{1,0,0.25},{0,1,0.25},{1,1,0.25}}};
            bp={{{0,0,0},{1,0,0},{0,1,0},{1,1,1}}};
        } else bp={{{0,0,1},{1,0,1},{0,1,1},{1,1,0}}};
        Input a(ap,chart,plane,kind==6),b(bp,!chart,plane);
        const auto av=a.view(),bv=b.view();const auto before_a=a.controls,before_b=b.controls;
        const auto before_au=a.knots[0],before_av=a.knots[1],before_bu=b.knots[0],before_bv=b.knots[1];
        const double tolerance=kind==6?1e-6:1e-8;
        VbSurfaceIntersections *events=nullptr;char error[1024]={};
        int status=vb_surface_intersect(&av,&bv,tolerance,0.1,&events,error,sizeof(error));
        const size_t count=vb_surface_intersections_count(events);
        std::vector<std::array<VbSurfaceData*,3>> snapshots(count);
        std::vector<int> types(count);std::vector<Point> points(count);
        for(size_t i=0;i<count && !status;++i) {
            if(vb_surface_intersections_info(events,i,&types[i],points[i].data()))status=2;
            for(size_t part=0;part<3;++part) {
                const int take_status=vb_surface_intersections_take_curve(events,i,part,&snapshots[i][part]);
                if(types[i]==1) {if(!take_status || snapshots[i][part])status=2;}
                else if(take_status || !snapshots[i][part])status=2;
                auto *again=snapshots[i][part];
                if(vb_surface_intersections_take_curve(events,i,part,&again)==0 || again)status=2;
            }
        }
        VbSurfaceData *invalid_snapshot=nullptr;
        if(vb_surface_intersections_take_curve(events,count,0,&invalid_snapshot)==0 || invalid_snapshot)status=2;
        if(vb_surface_intersections_take_curve(events,0,3,&invalid_snapshot)==0 || invalid_snapshot)status=2;
        for(double value:{0.,-1.,std::numeric_limits<double>::infinity(),std::numeric_limits<double>::quiet_NaN()}) {
            auto *invalid=events;
            if(!vb_surface_intersect(&av,&bv,value,0.1,&invalid,error,sizeof(error)) || invalid)status=2;
            invalid=events;
            if(!vb_surface_intersect(&av,&bv,tolerance,value,&invalid,error,sizeof(error)) || invalid)status=2;
        }
        for(size_t degree:{size_t(0),size_t(18)}) {
            auto malformed=av;malformed.degree[0]=degree;auto *invalid=events;
            if(!vb_surface_intersect(&malformed,&bv,tolerance,0.1,&invalid,error,sizeof(error)) || invalid)status=2;
        }
        // Exported snapshots must remain valid after destroying the event owner.
        vb_surface_intersections_free(events);
        VbSurfaceIntersections *untaken=nullptr;
        if(vb_surface_intersect(&av,&bv,tolerance,0.1,&untaken,error,sizeof(error)))status=2;
        vb_surface_intersections_free(untaken);
        double residual=0;
        for(size_t i=0;i<count;++i) {
            if(types[i]==1) {
                if(kind!=3 || gap(points[i],rotate({1,1,0},plane))>tolerance)status=2;
                continue;
            }
            std::array<VbCurve*,3> curves{};
            std::array<std::array<double,2>,3> domains{};
            for(size_t part=0;part<3;++part) {
                VbNurbsData view{};
                if(vb_surface_data_view(snapshots[i][part],&view)) {status=2;continue;}
                domains[part]={view.knots[0][view.degree[0]],view.knots[0][view.count[0]]};
                if(vb_curve_create(view.degree[0],view.points_xyzw,view.count[0],view.knots[0],view.knot_count[0],&curves[part],error,sizeof(error)))status=2;
                vb_surface_data_free(snapshots[i][part]);
            }
            for(int station=0;station<=64 && !status;++station) {
                const double f=station/64.;std::array<Point,3> q{};
                for(size_t part=0;part<3;++part)
                    if(vb_curve_evaluate(curves[part],domains[part][0]*(1-f)+domains[part][1]*f,q[part].data(),error,sizeof(error)))status=2;
                const double error_value=std::max(gap(q[0],a.image(q[1][0],q[1][1],plane)),gap(q[0],b.image(q[2][0],q[2][1],plane)));
                if(!std::isfinite(error_value))status=2;else residual=std::max(residual,error_value);
            }
            for(auto *curve:curves)vb_curve_free(curve);
        }
        const bool valid=!status && (kind==4?(count==2 || count==4):count==size_t(kind==1?0:kind==2?4:1))
            && residual<=tolerance && a.controls==before_a && b.controls==before_b
            && a.knots[0]==before_au && a.knots[1]==before_av && b.knots[0]==before_bu && b.knots[1]==before_bv;
        passed=passed&&valid;
        if(comma)std::cout<<',';comma=true;
        std::cout<<"{\"kind\":"<<kind<<",\"chart\":"<<chart<<",\"plane\":"<<plane<<",\"status\":"<<status
            <<",\"count\":"<<count<<",\"residual\":"<<residual<<",\"passed\":"<<(valid?"true":"false")<<'}';
    }
    VbSurfaceIntersections *invalid=nullptr;char error[1024]={};
    const bool invalid_input=vb_surface_intersect(nullptr,nullptr,1e-8,0.1,&invalid,error,sizeof(error))!=0 && !invalid;
    passed=passed&&invalid_input;
    std::cout<<"],\"invalid_input_rejected\":"<<(invalid_input?"true":"false")<<",\"passed\":"<<(passed?"true":"false")<<"}\n";
    return passed?0:1;
}
