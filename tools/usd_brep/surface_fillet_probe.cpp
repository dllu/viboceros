#include "bridge.h"
#include <array>
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <vector>
namespace {
void check(bool ok) { if(!ok) throw std::runtime_error("surface fillet probe invariant"); }
struct Surface {
    size_t p=1,q=1,nu=2,nv=2;
    std::vector<double> u{0,0,1,1},v{0,0,1,1},points;
    VbNurbsData view()const { return {{p,q},{nu,nv},{u.size(),v.size()},{u.data(),v.data()},points.data()}; }
    bool operator==(const Surface&b)const{return p==b.p && q==b.q && nu==b.nu && nv==b.nv && u==b.u && v==b.v && points==b.points;}
};
Surface plane(bool second,double start,double end) {
    Surface s;
    for(double y:{start,end}) for(double x:{0.,10.}) {
        if(second) s.points.insert(s.points.end(),{5.,y,x-5.,1.});
        else s.points.insert(s.points.end(),{x,y,0.,1.});
    }
    return s;
}
Surface cylinder() {
    Surface s;s.p=2;s.nu=3;s.u={0,0,0,1,1,1};
    for(double z:{-5.,5.}) for(int i=0;i<3;++i)
        s.points.insert(s.points.end(),{i<2?5.:0.,i?5.:0.,z,i==1?std::sqrt(.5):1.});
    return s;
}
void array(const double *p,size_t n) {
    std::cout<<'[';for(size_t i=0;i<n;++i){if(i)std::cout<<',';if(std::isfinite(p[i]))std::cout<<p[i];else std::cout<<"null";}std::cout<<']';
}
bool print(const VbNurbsData&n) {
    check(n.count[0] && n.count[1] && n.count[0]<=1000000/n.count[1]);
    std::cout<<"{\"degree\":["<<n.degree[0]<<','<<n.degree[1]<<"],\"count\":["<<n.count[0]<<','<<n.count[1]<<"],\"knots\":[";
    array(n.knots[0],n.knot_count[0]);std::cout<<',';array(n.knots[1],n.knot_count[1]);std::cout<<"],\"points_xyzw\":";
    array(n.points_xyzw,4*n.count[0]*n.count[1]);std::cout<<'}';
    for(size_t i=0;i<4*n.count[0]*n.count[1];++i)if(!std::isfinite(n.points_xyzw[i]))return false;
    return true;
}
size_t rejects() {
    size_t failures=0;
    for(int mode=0;mode<10;++mode) {
        auto a=plane(false,0,10),b=plane(true,0,10);const auto beforeA=a,beforeB=b;
        auto av=a.view(),bv=b.view();double radius=1.,factor=.5,accuracy=1e-8;int extend=0,sides[2]={1,1};
        if(mode==0)radius=0;
        if(mode==1)radius=-1;
        if(mode==2)radius=NAN;
        if(mode==3)radius=INFINITY;
        if(mode==4)sides[0]=0;
        if(mode==5)extend=2;
        if(mode==6)factor=-1;
        if(mode==7)factor=NAN;
        if(mode==8)accuracy=0;
        VbSolid *out=reinterpret_cast<VbSolid*>(uintptr_t(1));char error[2048]={};
        const int status=vb_surface_fillet(mode==9?nullptr:&av,&bv,radius,sides,extend,factor,accuracy,&out,error,sizeof error);
        const bool passed=status!=0 && out==nullptr && a==beforeA && b==beforeB && error[0];failures+=!passed;
        std::cout<<"{\"invalid\":true,\"case\":"<<mode<<",\"status\":"<<status<<",\"passed\":"<<passed<<"}\n";
    }
    return failures;
}
}
int main() {
 try {
    std::cout<<std::setprecision(17)<<std::boolalpha;size_t total=0,failures=0;
    for(int family=0;family<4;++family) for(int side1:{-1,1})for(int side2:{-1,1})
    for(double radius:{.25,1.})for(int extend=0;extend<(family==3?1:2);++extend) {
        ++total;const int sides[2]={side1,side2};VbSolid *raw=nullptr;char error[2048]={};int status;bool unchanged;
        const double factor=extend?(family==0?.5:family==1?1.:256.):0.;
        {
            auto a=plane(false,0,10),b=family==3?cylinder():plane(true,family==0?0.:family==1?3.:4.99,family==0?10.:family==1?7.:5.01);
            if(family==3)a.points={-10,-10,0,1,10,-10,0,1,-10,10,0,1,10,10,0,1};
            const auto beforeA=a,beforeB=b;const auto av=a.view(),bv=b.view();
            status=vb_surface_fillet(&av,&bv,radius,sides,extend,factor,1e-8,&raw,error,sizeof error);
            unchanged=a==beforeA && b==beforeB;
        }
        // Both source buffers and both private native parent bodies are gone.
        std::unique_ptr<VbSolid,decltype(&vb_solid_free)> model(raw,&vb_solid_free);
        bool passed=status==0 && model && unchanged;VbBrep *snapshot=nullptr;int exported=-1;
        if(model)exported=vb_shape_brep(model.get(),1e-10,&snapshot,error,sizeof error);
        std::unique_ptr<VbBrep,decltype(&vb_brep_free)> data(snapshot,&vb_brep_free);
        passed &= exported==0 && data!=nullptr;
        model.reset(); // Serialized output must also outlive its native model.
        std::cout<<"{\"family\":"<<family<<",\"sides\":["<<side1<<','<<side2<<"],\"radius\":"<<radius<<",\"extend\":"<<bool(extend)<<",\"extension_factor\":"<<factor<<",\"status\":"<<status<<",\"export_status\":"<<exported<<",\"source_unchanged\":"<<unchanged;
        if(data) {
            VbBrepView view{};check(vb_brep_view(data.get(),&view)==0);
            passed &= view.face_count>0 && view.vertex_count>0 && view.edge_count>0;
            std::cout<<",\"counts\":["<<view.face_count<<','<<view.edge_count<<','<<view.vertex_count<<"],\"vertices\":[";
            for(size_t i=0;i<view.vertex_count;++i){if(i)std::cout<<',';array(view.vertices[i].point,3);for(double x:view.vertices[i].point)passed &=std::isfinite(x);}
            std::cout<<"],\"surfaces\":[";
            for(size_t i=0;i<view.face_count;++i){if(i)std::cout<<',';passed &= print(view.faces[i].surface);}
            std::cout<<"],\"edges\":[";
            for(size_t i=0;i<view.edge_count;++i){if(i)std::cout<<',';passed &= print(view.edges[i].curve);}
            std::cout<<"],\"trims\":[";
            for(size_t i=0;i<view.trim_count;++i){if(i)std::cout<<',';passed &= print(view.trims[i].curve);}
            std::cout<<']';
            double maximum_error=0,min_y=INFINITY,max_y=-INFINITY;
            const auto geometry_error=[&](const double p[3]) {
                const double center_z=side1*radius;
                const double radial=family==3?std::hypot(p[0],p[1])-(5.+side2*radius):p[0]-(5.-side2*radius);
                const double delta=std::abs(std::hypot(radial,p[2]-center_z)-radius);
                maximum_error=std::max(maximum_error,delta);
            };
            for(size_t i=0;i<view.vertex_count;++i) {
                geometry_error(view.vertices[i].point);
                min_y=std::min(min_y,view.vertices[i].point[1]);max_y=std::max(max_y,view.vertices[i].point[1]);
            }
            for(size_t i=0;i<view.edge_count;++i) {
                const auto &edge=view.edges[i];const auto &c=edge.curve;VbCurve *curve=nullptr;
                check(vb_curve_create(c.degree[0],c.points_xyzw,c.count[0],c.knots[0],c.knot_count[0],&curve,error,sizeof error)==0);
                std::unique_ptr<VbCurve,decltype(&vb_curve_free)> curve_owner(curve,&vb_curve_free);
                for(int j=0;j<=4;++j) {double p[3];const double u=edge.interval[0]+(edge.interval[1]-edge.interval[0])*j/4.;
                    check(vb_curve_evaluate(curve,u,p,error,sizeof error)==0);geometry_error(p);}
            }
            if(family!=3) {
                const double expected_min=extend || family==0?0.:family==1?3.:4.99;
                const double expected_max=extend || family==0?10.:family==1?7.:5.01;
                maximum_error=std::max(maximum_error,std::max(std::abs(min_y-expected_min),std::abs(max_y-expected_max)));
            }
            passed &= std::isfinite(maximum_error) && maximum_error<=1e-8;
            std::cout<<",\"maximum_geometry_error\":"<<maximum_error;
        }
        failures+=!passed;std::cout<<",\"passed\":"<<passed<<"}\n";
    }
    failures+=rejects();std::cerr<<total<<" surface fillet lifetime cases, 10 rejection cases, "<<failures<<" failures\n";
    return failures?1:0;
 }catch(const std::exception&e){std::cerr<<e.what()<<'\n';return 2;}
}
