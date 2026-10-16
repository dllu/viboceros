#include "bridge.h"
#include <array>
#include <cmath>
#include <iostream>
#include <limits>
#include <memory>
#include <stdexcept>
#include <vector>
namespace {
using Curve=std::unique_ptr<VbCurve,decltype(&vb_curve_free)>;
void require(bool ok,const char *why) {if(!ok) throw std::runtime_error(why);}
Curve curve(size_t degree,const std::vector<double>&p,const std::vector<double>&k) {
    VbCurve *out=nullptr;char error[2048]{};
    require(vb_curve_create(degree,p.data(),p.size()/4,k.data(),k.size(),&out,error,sizeof(error))==0,error);
    return Curve(out,vb_curve_free);
}
std::array<double,2> query(VbCurve *a,VbCurve *b,const std::array<double,4>&ranges) {
    double params[4]{};char error[2048]{};
    require(vb_curve_distance_extrema(a,b,ranges.data(),1e-9,params,error,sizeof(error))==0,error);
    std::array<double,2> distance{};
    for(size_t i=0;i<2;++i) {
        double p[3]{},q[3]{};
        require(vb_curve_evaluate(a,params[2*i],p,error,sizeof(error))==0,error);
        require(vb_curve_evaluate(b,params[2*i+1],q,error,sizeof(error))==0,error);
        distance[i]=std::hypot(std::hypot(p[0]-q[0],p[1]-q[1]),p[2]-q[2]);
    }
    return distance;
}
}
int main() {
    try {
        auto a=curve(1,{0,0,0,1, 1,0,0,1},{0,0,1,1});
        auto b=curve(1,{0,.2,0,1, 1,.2,0,1},{0,0,1,1});
        const auto parallel=query(a.get(),b.get(),{0,1,0,1});
        require(std::abs(parallel[0]-.2)<1e-8&&std::abs(parallel[1]-.2)<1e-8,"parallel extrema");
        auto c=curve(1,{.3,.2,0,1, .7,.2,0,1},{0,0,1,1});
        const auto partial=query(a.get(),c.get(),{.3,.7,0,1});
        require(std::abs(partial[0]-.2)<1e-8&&std::abs(partial[1]-.2)<1e-8,"partial extrema");
        auto chord=curve(1,{-4,0,0,1, 4,0,0,1},{0,0,1,1});
        const double w=std::sqrt(.5);
        auto ring=curve(2,{2,0,0,1, 2,2,0,w, 0,2,0,1, -2,2,0,w, -2,0,0,1,
            -2,-2,0,w, 0,-2,0,1, 2,-2,0,w, 2,0,0,1},
            {0,0,0,.25,.25,.5,.5,.75,.75,1,1,1});
        const auto circular=query(chord.get(),ring.get(),{0,1,0,1});
        require(std::abs(circular[0]-2)<1e-8&&circular[1]<1e-8,"circle/chord nearest-branch extrema");
        const double delta=1./4096.;
        auto peak=curve(2,{0,0,0,1, 2.5,0,0,1, 5,0,0,1, 5+5*delta,2,0,1,
            5+10*delta,0,0,1, 7.5,0,0,1, 10,0,0,1},
            {0,0,0,.5,.5,.5+delta,.5+delta,1,1,1});
        auto axis=curve(1,{0,0,0,1, 10,0,0,1},{0,0,1,1});
        const auto localized=query(peak.get(),axis.get(),{0,1,0,1});
        require(std::abs(localized[0]-1)<1e-8,"localized knot-span peak missed");
        char error[2048]{};double params[4]{};const double bad[4]={-1,1,0,1};
        require(vb_curve_distance_extrema(a.get(),b.get(),bad,1e-9,params,error,sizeof(error))!=0,"invalid range accepted");
        const double tiny_bad[4]={-std::numeric_limits<double>::epsilon(),1,0,1};
        require(vb_curve_distance_extrema(a.get(),b.get(),tiny_bad,1e-9,params,error,sizeof(error))!=0,
            "small invalid caller range accepted by output roundoff guard");
        require(vb_curve_distance_extrema(nullptr,b.get(),bad,1e-9,nullptr,error,sizeof(error))!=0,"null arguments accepted");
        std::cout<<"{\"passed\":true,\"checks\":7,\"parallel_maximum\":"<<parallel[0]
                 <<",\"partial_maximum\":"<<partial[0]<<",\"circle_chord_maximum\":"<<circular[0]
                 <<",\"circle_chord_minimum\":"<<circular[1]<<"}\n";
    } catch(const std::exception &error) {std::cerr<<error.what()<<'\n';return 1;}
}
