// Actual C ABI tangent-circle UV contact diagnostics with independent images.
#include <bridge.h>
#include <array>
#include <algorithm>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <vector>
using Point=std::array<double,3>;
Point rotate(Point p,int plane) {
    if(plane==1)return {p[2],p[0],p[1]};
    if(plane==2)return {p[1],p[2],p[0]};
    return p;
}
double y_at(double x) {
    double lo=0,hi=1;
    for(int i=0;i<64;++i) {const double y=(lo+hi)*0.5;if(x*x+y*y*y+x*y<1)lo=y;else hi=y;}
    return (lo+hi)*0.5;
}
double gap(Point a,Point b) {return std::hypot(std::hypot(a[0]-b[0],a[1]-b[1]),a[2]-b[2]);}
int main() {
    bool passed=true,comma=false;
    std::cout<<std::unitbuf<<std::setprecision(17)<<"{\"cases\":[";
    for(int chart=0;chart<2;++chart)for(int plane=0;plane<3;++plane)for(int part=1;part<=2;++part)for(double side:{-1.,1.}) {
        const double au0=chart?-4:0,au1=chart?7:1,av0=chart?10:0,av1=chart?16:1;
        const double bu0=chart?30:0,bu1=chart?90:1,bv0=chart?-8:0,bv1=chart?3:1;
        const double aku[]={au0,au0,au0,au1,au1,au1},akv[]={av0,av0,av0,av0,av1,av1,av1,av1};
        const double bku[]={bu0,bu0,bu1,bu1},bkv[]={bv0,bv0,bv1,bv1};
        std::vector<double> ap,bp;
        for(int v=0;v<4;++v)for(int u=0;u<3;++u) {
            const auto p=rotate({u/2.,v/3.,double(u==2)+double(v==3)},plane);
            ap.insert(ap.end(),{p[0],p[1],p[2],1});
        }
        for(int v=0;v<2;++v)for(int u=0;u<2;++u) {
            const auto p=rotate({double(u),double(v),1.-u*v},plane);
            bp.insert(bp.end(),{p[0],p[1],p[2],1});
        }
        const auto before_a=ap,before_b=bp;
        const VbNurbsData a={{2,3},{3,4},{6,8},{aku,akv},ap.data()},b={{1,1},{2,2},{4,4},{bku,bkv},bp.data()};
        char error[1024]={};VbSurfaceIntersections *events=nullptr;
        int status=vb_surface_intersect(&a,&b,1e-6,0.1,&events,error,sizeof(error));
        if(vb_surface_intersections_count(events)!=1)status=2;
        std::array<VbSurfaceData*,3> snapshots{};
        std::array<VbCurve*,3> curves{};
        std::array<std::array<double,2>,3> intervals{};
        for(size_t i=0;i<3;++i)if(!status) {
            if(vb_surface_intersections_take_curve(events,0,i,&snapshots[i]))status=2;
        }
        vb_surface_intersections_free(events);
        for(size_t i=0;i<3;++i) {
            VbNurbsData view{};
            if(vb_surface_data_view(snapshots[i],&view)) {status=2;continue;}
            intervals[i]={view.knots[0][view.degree[0]],view.knots[0][view.count[0]]};
            std::vector<double> controls(view.points_xyzw,view.points_xyzw+view.count[0]*4);
            if(i) {
                const double u0=i==1?au0:bu0,u1=i==1?au1:bu1,v0=i==1?av0:bv0,v1=i==1?av1:bv1;
                for(size_t c=0;c<view.count[0];++c) {controls[4*c]=(controls[4*c]-u0)/(u1-u0);controls[4*c+1]=(controls[4*c+1]-v0)/(v1-v0);}
            }
            if(vb_curve_create(view.degree[0],controls.data(),view.count[0],view.knots[0],view.knot_count[0],&curves[i],error,sizeof(error)))status=2;
            vb_surface_data_free(snapshots[i]);
        }
        double model_error=0,uv_error=0;size_t contacts=0;
        const double target=0.45,y=y_at(target),radius=0.06;
        const double gx=2*target+y,gy=3*y*y+target,length=std::hypot(gx,gy);
        const double cx=target+side*radius*gx/length,cy=y+side*radius*gy/length;
        const double weights[]={1,std::sqrt(0.5),1,std::sqrt(0.5),1,std::sqrt(0.5),1,std::sqrt(0.5),1};
        const double offsets[][2]={{1,0},{1,1},{0,1},{-1,1},{-1,0},{-1,-1},{0,-1},{1,-1},{1,0}};
        std::vector<double> controls;
        for(int i=0;i<9;++i)controls.insert(controls.end(),{cx+radius*offsets[i][0],cy+radius*offsets[i][1],0,weights[i]});
        const double knots[]={0,0,0,1,1,2,2,3,3,4,4,4};
        VbCurve *boundary=nullptr;VbCurveIntersections *hits=nullptr;
        if(vb_curve_create(2,controls.data(),9,knots,12,&boundary,error,sizeof(error)))status=2;
        if(vb_curve_intersect(curves[part],boundary,1e-12,&hits,error,sizeof(error)))status=2;
        const size_t count=vb_curve_intersections_count(hits);
        if(count>8)status=2;
        std::vector<VbCurveIntersection> records(count);
        if(vb_curve_intersections_copy(hits,records.data(),count))status=2;
        for(const auto &hit:records) {
            if(hit.overlap)status=2;
            Point xyz,uv,edge;
            const double fraction=(hit.first[0]-intervals[part][0])/(intervals[part][1]-intervals[part][0]);
            if(vb_curve_evaluate(curves[0],intervals[0][0]*(1-fraction)+intervals[0][1]*fraction,xyz.data(),error,sizeof(error)))status=2;
            if(vb_curve_evaluate(curves[part],hit.first[0],uv.data(),error,sizeof(error)))status=2;
            if(vb_curve_evaluate(boundary,hit.second[0],edge.data(),error,sizeof(error)))status=2;
            const double me=gap(xyz,rotate({edge[0],edge[1],1-edge[0]*edge[1]},plane));
            const double ue=std::max(gap(uv,edge),std::abs(std::hypot(edge[0]-cx,edge[1]-cy)-radius));
            if(!std::isfinite(me) || !std::isfinite(ue))status=2;
            else {model_error=std::max(model_error,me);uv_error=std::max(uv_error,ue);}
            ++contacts;
        }
        vb_curve_intersections_free(hits);vb_curve_free(boundary);
        for(auto *curve:curves)vb_curve_free(curve);
        const bool valid=!status && model_error<=1e-6 && uv_error<=1e-12 && ap==before_a && bp==before_b;
        passed=passed&&valid;
        if(comma)std::cout<<',';comma=true;
        std::cout<<"{\"chart\":"<<chart<<",\"plane\":"<<plane<<",\"part\":"<<part<<",\"side\":"<<side<<",\"status\":"<<status
            <<",\"contacts\":"<<contacts<<",\"model_error\":"<<model_error<<",\"uv_error\":"<<uv_error<<",\"passed\":"<<(valid?"true":"false")<<'}';
    }
    std::cout<<"],\"passed\":"<<(passed?"true":"false")<<"}\n";return passed?0:1;
}
