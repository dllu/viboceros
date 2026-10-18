#include "bridge.h"
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <vector>
namespace {
void check(bool valid,const char *message) {if(!valid)throw std::runtime_error(message);}
struct Input {
    size_t degree=1,count=2;
    std::vector<double>u{0,0,1,1},v{0,0,1,1},points;
    VbNurbsData view()const{return {{degree,1},{count,2},{u.size(),v.size()},{u.data(),v.data()},points.data()};}
    bool operator==(const Input&b)const{return degree==b.degree && count==b.count && u==b.u && v==b.v && points==b.points;}
};
Input plane(bool second,bool large) {
    Input s;for(double y:{large?-10.:0.,10.})for(double x:{large?-10.:0.,10.}) {
        if(second)s.points.insert(s.points.end(),{5.,y,x-5.,1.});
        else s.points.insert(s.points.end(),{x,y,0.,1.});
    }return s;
}
Input cylinder(bool closed) {
    Input s;s.degree=2;s.count=closed?9:3;
    s.u=closed?std::vector<double>{0,0,0,1,1,2,2,3,3,4,4,4}:std::vector<double>{0,0,0,1,1,1};
    const double xy[9][2]={{5,0},{5,5},{0,5},{-5,5},{-5,0},{-5,-5},{0,-5},{5,-5},{5,0}};
    for(double z:{-5.,5.})for(size_t i=0;i<s.count;++i)
        s.points.insert(s.points.end(),{xy[i][0],xy[i][1],z,i%2?std::sqrt(.5):1.});
    return s;
}
void numbers(const double*p,size_t n) {std::cout<<'[';for(size_t i=0;i<n;++i){if(i)std::cout<<',';check(std::isfinite(p[i]),"nonfinite output");std::cout<<p[i];}std::cout<<']';}
void nurbs(const VbNurbsData&n) {
    std::cout<<"{\"degree\":["<<n.degree[0]<<','<<n.degree[1]<<"],\"count\":["<<n.count[0]<<','<<n.count[1]<<"],\"knots\":[";
    numbers(n.knots[0],n.knot_count[0]);std::cout<<',';numbers(n.knots[1],n.knot_count[1]);std::cout<<"],\"points_xyzw\":";numbers(n.points_xyzw,4*n.count[0]*n.count[1]);std::cout<<'}';
}
}
int main() {
 try {
    std::cout<<std::setprecision(17)<<std::boolalpha;size_t cases=0;
    for(int family=0;family<3;++family)for(int a:{-1,1})for(int b:{-1,1})for(double radius:{.25,.75})for(int trim:{0,1}) {
        ++cases;VbSolid *out[3]={};int changed[2]={};char error[2048]={};int status;bool unchanged;
        {
            auto first=plane(false,family!=0),second=family?cylinder(family==2):plane(true,false);
            const auto saved_first=first,saved_second=second;const auto f=first.view(),s=second.view();const int sides[2]={a,b};
            status=vb_surface_fillet_parents(&f,&s,radius,sides,0,0,1e-8,trim,out,changed,error,sizeof error);
            unchanged=first==saved_first && second==saved_second;
        }
        std::unique_ptr<VbSolid,decltype(&vb_solid_free)>owners[3]={{out[0],&vb_solid_free},{out[1],&vb_solid_free},{out[2],&vb_solid_free}};
        check(status==0,error);check(unchanged && owners[0],"source purity or missing fillet");
        check(changed[0]==(family!=1) && changed[1]==1,"parent changed flags");
        VbBrep *copies[3]={};
        for(size_t i=0;i<3;++i)if(owners[i])check(vb_shape_brep(owners[i].get(),1e-10,&copies[i],error,sizeof error)==0,error);
        // All sources, private native parents, and all three result handles die.
        for(auto &owner:owners)owner.reset();
        std::cout<<"{\"family\":"<<family<<",\"sides\":["<<a<<','<<b<<"],\"radius\":"<<radius<<",\"trim\":"<<bool(trim)<<",\"source_unchanged\":"<<unchanged<<",\"changed\":["<<changed[0]<<','<<changed[1]<<"],\"outputs\":[";
        double maximum_error=0;
        for(size_t i=0;i<3;++i) {
            if(i)std::cout<<',';
            std::unique_ptr<VbBrep,decltype(&vb_brep_free)>copy(copies[i],&vb_brep_free);
            if(!copy){check(i==1 && family==1,"unexpected absent parent");std::cout<<"null";continue;}
            VbBrepView view{};check(vb_brep_view(copy.get(),&view)==0,"snapshot view");
            check(view.face_count==(i==0 || trim?1u:2u),"face count");
            std::cout<<"{\"counts\":["<<view.face_count<<','<<view.edge_count<<','<<view.vertex_count<<"],\"vertices\":[";
            for(size_t j=0;j<view.vertex_count;++j){if(j)std::cout<<',';numbers(view.vertices[j].point,3);}
            std::cout<<"],\"surfaces\":[";for(size_t j=0;j<view.face_count;++j){if(j)std::cout<<',';nurbs(view.faces[j].surface);}
            std::cout<<"],\"edges\":[";
            for(size_t j=0;j<view.edge_count;++j) {
                if(j)std::cout<<',';
                const auto&e=view.edges[j];nurbs(e.curve);
                VbCurve *raw=nullptr;check(vb_curve_create(e.curve.degree[0],e.curve.points_xyzw,e.curve.count[0],e.curve.knots[0],e.curve.knot_count[0],&raw,error,sizeof error)==0,error);
                std::unique_ptr<VbCurve,decltype(&vb_curve_free)>curve(raw,&vb_curve_free);
                for(int k=0;k<=16;++k) {
                    const double t=k==0?e.interval[0]:k==16?e.interval[1]:e.interval[0]+(e.interval[1]-e.interval[0])*k/16.;double p[3];check(vb_curve_evaluate(raw,t,p,error,sizeof error)==0,error);
                    if(i==0){const double radial=family?std::hypot(p[0],p[1])-(5+b*radius):p[0]-(5-b*radius);maximum_error=std::max(maximum_error,std::abs(std::hypot(radial,p[2]-a*radius)-radius));}
                }
            }
            std::cout<<"],\"trims\":[";for(size_t j=0;j<view.trim_count;++j){if(j)std::cout<<',';nurbs(view.trims[j].curve);}std::cout<<"]}";
        }
        check(maximum_error<=1e-8,"fillet radius identity");std::cout<<"],\"maximum_geometry_error\":"<<maximum_error<<",\"passed\":true}\n";
    }
    for(int mode=0;mode<8;++mode) {
        auto first=plane(false,false),second=plane(true,false);const auto f=first.view(),s=second.view();int sides[2]={1,1},changed[2]={-1,-1};VbSolid *out[3]={reinterpret_cast<VbSolid*>(uintptr_t(1)),reinterpret_cast<VbSolid*>(uintptr_t(1)),reinterpret_cast<VbSolid*>(uintptr_t(1))};char error[2048]={};
        const int status=vb_surface_fillet_parents(mode==0?nullptr:&f,&s,mode==1?0.:1.,mode==2?nullptr:sides,mode==3?2:0,0,mode==4?0.:1e-8,mode==5?2:0,mode==6?nullptr:out,mode==7?nullptr:changed,error,sizeof error);
        check(status!=0 && error[0],"invalid request accepted");if(mode!=6)for(auto*p:out)check(!p,"partial output on failure");if(mode!=7)for(auto x:changed)check(x==0,"changed flags on failure");std::cout<<"{\"invalid\":true,\"case\":"<<mode<<",\"passed\":true}\n";
    }
    std::cerr<<cases<<" parent fillet lifetime cases, 8 rejection cases, 0 failures\n";return 0;
 }catch(const std::exception&e){std::cerr<<e.what()<<'\n';return 2;}
}
