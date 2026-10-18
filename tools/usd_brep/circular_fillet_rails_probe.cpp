// Same-parameter rail correspondence, independent of closest-point queries.
#include <SmApiGeneral.h>
#include <SmBSplineCurve.h>
#include <SmBSplineSurface.h>
#include <SmFilletSolver.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>

namespace {
void check(SmStatus status) {
    if (status != SM_SUCCESS) throw std::runtime_error("circular fillet rail probe setup failed");
}
SmPoint3d place(SmPoint3d p, double scale, bool rotated) {
    return rotated ? SmPoint3d((p.z+3)*scale,(p.x-2)*scale,(p.y+4)*scale)
                   : SmPoint3d(p.x*scale,p.y*scale,p.z*scale);
}
SmPoint3d local(SmPoint3d p, double scale, bool rotated) {
    return rotated ? SmPoint3d(p.y/scale+2,p.z/scale-4,p.x/scale-3)
                   : SmPoint3d(p.x/scale,p.y/scale,p.z/scale);
}
std::array<SmPoint3d,3> polygon(double radius, double z, double angle, bool reversed) {
    std::array<SmPoint3d,3> p{{{radius,0,z},{radius,radius*std::tan(angle/2),z},
                              {radius*std::cos(angle),radius*std::sin(angle),z}}};
    if (reversed) std::reverse(p.begin(),p.end());
    return p;
}
std::unique_ptr<SmBSplineCurve> arc(double radius, double z, double angle, double k,
                                  bool reversed, double start, double end,
                                  double scale, bool rotated) {
    SmTArray<SmPoint3d> points;
    for (auto p : polygon(radius,z,angle,reversed)) points.Add(place(p,scale,rotated));
    SmTArray<double> weights,knots;
    weights.Add(1);weights.Add(k*std::cos(angle/2));weights.Add(k*k);
    knots.Add(start);knots.Add(end);
    SmTArray<ULONG> multiplicities;multiplicities.Add(3);multiplicities.Add(3);
    SmBSplineCurve *raw=nullptr;
    check(SmBSplineCurve::CreateCanonical(*SmApiGetOrCreateContext(),3,2,points,
        SM_CF_UNSPECIFIED,multiplicities,knots,SM_KT_UNSPECIFIED,&weights,nullptr,raw));
    return std::unique_ptr<SmBSplineCurve>(raw);
}
// Independent homogeneous quadratic Bernstein evaluation of the source rail.
SmPoint3d reference(double radius,double z,double angle,double k,bool reversed,
                    double t,double scale,bool rotated) {
    const auto p=polygon(radius,z,angle,reversed);
    const std::array<double,3> b{{(1-t)*(1-t),2*t*(1-t)*k*std::cos(angle/2),t*t*k*k}};
    const double w=b[0]+b[1]+b[2];
    return place(SmPoint3d((b[0]*p[0].x+b[1]*p[1].x+b[2]*p[2].x)/w,
                           (b[0]*p[0].y+b[1]*p[1].y+b[2]*p[2].y)/w,z),scale,rotated);
}
}
int main() {
    try {
        std::cout<<std::setprecision(17)<<std::boolalpha;
        size_t cases=0,failures=0;
        for(double angle : {std::acos(-1.)/6,std::acos(-1.)/3,std::acos(-1.)/2})
        for(double k : {.5,1.,2.}) for(bool reversed : {false,true})
        for(bool shifted : {false,true}) for(double scale : {.1,1.,10.})
        for(bool rotated : {false,true}) {
            ++cases;const double start=shifted?-3.:0.,end=shifted?7.:1.;
            const double tolerance=1e-8*scale;
            auto center=arc(6,1,angle,k,reversed,start,end,scale,rotated);
            auto rail0=arc(6,0,angle,k,reversed,start,end,scale,rotated);
            auto rail1=arc(5,1,angle,k,reversed,start,end,scale,rotated);
            SmCircularCrossSectionFSG generator(FALSE);
            SmBSplineSurface *raw=nullptr;
            const auto status=generator.CreateSurfaceFromCurves(*SmApiGetOrCreateContext(),
                *center,*rail0,*rail1,tolerance,FALSE,nullptr,nullptr,nullptr,nullptr,raw);
            std::unique_ptr<SmBSplineSurface> surface(raw);
            double maximum_pair_gap=0,maximum_source_gap=0,maximum_torus_error=0;
            bool finite=true;
            if(surface) {
                const auto domain=surface->GetNaturalUVDomain();
                for(int i=0;i<=64;++i) {
                    const double fraction=i/64.,t=start+(end-start)*fraction;
                    for(int rail=0;rail<2;++rail) {
                        SmPoint3d c,p;
                        check((rail?rail1:rail0)->EvaluatePoint(t,c));
                        check(surface->EvaluatePoint(SmPoint2d(t,rail?domain.GetMax().y:domain.GetMin().y),p));
                        const double pair_gap=c.DistanceBetween(p);
                        const double source_gap=c.DistanceBetween(reference(
                            rail?5:6,rail?1:0,angle,k,reversed,fraction,scale,rotated));
                        finite &= std::isfinite(pair_gap) && std::isfinite(source_gap);
                        maximum_pair_gap=std::max(maximum_pair_gap,pair_gap);
                        maximum_source_gap=std::max(maximum_source_gap,source_gap);
                    }
                }
                center.reset();rail0.reset();rail1.reset(); // Output owns all its data.
                SmTArray<double> breaks;check(surface->GetKnots(SM_SP_U,breaks));
                for(ULONG i=0;i+1<breaks.GetSize();++i) for(int j=0;j<=4;++j)
                for(int v=0;v<=8;++v) {
                    SmPoint3d p;
                    check(surface->EvaluatePoint(SmPoint2d(breaks[i]+(breaks[i+1]-breaks[i])*j/4.,
                        domain.GetVInterval().Evaluate(v/8.)),p));
                    p=local(p,scale,rotated);
                    const double error=scale*std::abs(std::hypot(std::hypot(p.x,p.y)-6,p.z-1)-1);
                    finite &= std::isfinite(error);
                    maximum_torus_error=std::max(maximum_torus_error,error);
                }
            }
            const bool passed=status==SM_SUCCESS && surface && finite && maximum_pair_gap<=tolerance
                && maximum_source_gap<=tolerance && maximum_torus_error<=tolerance;
            failures+=!passed;
            std::cout<<"{\"angle\":"<<angle<<",\"mobius\":"<<k<<",\"reversed\":"<<reversed
                <<",\"shifted\":"<<shifted<<",\"scale\":"<<scale<<",\"rotated\":"<<rotated
                <<",\"status\":"<<status<<",\"tolerance\":"<<tolerance
                <<",\"maximum_pair_gap\":"<<maximum_pair_gap<<",\"maximum_source_gap\":"<<maximum_source_gap
                <<",\"maximum_torus_error\":"<<maximum_torus_error<<",\"passed\":"<<passed<<"}\n";
        }
        std::cerr<<cases<<" circular fillet rail cases, "<<failures<<" failures\n";
        return failures?1:0;
    } catch(const std::exception &e) {std::cerr<<e.what()<<'\n';return 2;}
}
