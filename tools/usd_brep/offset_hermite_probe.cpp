// Qualify the generic pointwise Hermite approximator on an offset curve.
#include <SmApiGeneral.h>
#include <SmBSplineCurve.h>
#include <SmOffsetCurve.h>
#include <algorithm>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>

int main() {
    SmApiCreateContext();std::cout<<std::setprecision(17)<<"{\"cases\":[";
    bool comma=false;
    for(double tolerance : {1e-5,1e-8,1e-10}) for(int chart : {0,1}) {
        const double lo=chart?-4:0,hi=chart?7:1;
        SmTArray<SmPoint3d> points;points.Add({0,0,0});points.Add({1,2,0});points.Add({2,0,0});
        SmTArray<double> knots;knots.Add(lo);knots.Add(hi);
        SmTArray<ULONG> multiplicities;multiplicities.Add(3);multiplicities.Add(3);
        SmBSplineCurve *raw=nullptr;
        auto status=SmBSplineCurve::CreateCanonical(*SmApiGetOrCreateContext(),3,2,points,
            SM_CF_UNSPECIFIED,multiplicities,knots,SM_KT_UNSPECIFIED,nullptr,nullptr,raw);
        std::unique_ptr<SmBSplineCurve> source(raw);
        SmBSplineCurve *output=nullptr;double reported=0,actual=0;
        if(status==SM_SUCCESS) {
            SmOffsetCurve offset(3,*source,source->GetNaturalInterval(),SmVector3d(0,0,1),-0.125);
            SmTArray<double> breaks;breaks.Add(lo);breaks.Add(hi);
            status=offset.ApproximateCurve(*SmApiGetOrCreateContext(),breaks,tolerance,reported,output,FALSE,TRUE,FALSE);
        }
        std::unique_ptr<SmBSplineCurve> result(output);
        if(status==SM_SUCCESS && result) {
            for(int i=0;i<=4096;++i) {
                const double u=i/4096.,t=lo+(hi-lo)*u,dy=4-8*u,speed=std::hypot(2.,dy);
                SmPoint3d p;
                if(result->EvaluatePoint(t,p)!=SM_SUCCESS) {status=SM_ERR;break;}
                const double error=std::hypot(std::hypot(p.x-(2*u-0.125*dy/speed),p.y-(4*u*(1-u)+0.25/speed)),p.z);
                if(!std::isfinite(error)) {status=SM_ERR;break;}
                actual=std::max(actual,error);
            }
        }
        if(comma)std::cout<<',';comma=true;
        std::cout<<"{\"tolerance\":"<<tolerance<<",\"chart\":"<<chart<<",\"status\":"<<status
            <<",\"reported_gap\":"<<reported<<",\"independent_point_error\":"<<actual;
        if(result)std::cout<<",\"controls\":"<<result->GetNumberControlPoints()<<",\"degree\":"<<result->GetDegree();
        std::cout<<'}';
    }
    std::cout<<"]}\n";
}
