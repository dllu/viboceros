// Qualify selected sides of the native curve-filleting executive.
#include <SmApiGeneral.h>
#include <SmBSplineCurve.h>
#include <SmCurveFilletExecutive.h>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>

class CornerFillet : public SmCurveFilletExecutive {
public:
    CornerFillet(SmBSplineCurve *a, SmBSplineCurve *b, double radius)
        : SmCurveFilletExecutive(a,b,radius,TRUE,FALSE) {
        m_dParam1=a->GetNaturalInterval().Evaluate(0.25);
        m_dParam2=b->GetNaturalInterval().Evaluate(0.75);
    }
};

std::unique_ptr<SmBSplineCurve> curve(bool line, double start, double end, double scale) {
    SmTArray<SmPoint3d> points;
    if(line) {points.Add({2*scale,2*scale,0});points.Add({4*scale,2*scale,0});}
    else {points.Add({0,0,0});points.Add({2*scale,0,0});points.Add({2*scale,2*scale,0});}
    SmTArray<double> knots;knots.Add(start);knots.Add(end);
    SmTArray<ULONG> multiplicities;multiplicities.Add(line?2:3);multiplicities.Add(line?2:3);
    SmBSplineCurve *raw=nullptr;
    if(SmBSplineCurve::CreateCanonical(*SmApiGetOrCreateContext(),3,line?1:2,points,
            SM_CF_UNSPECIFIED,multiplicities,knots,SM_KT_UNSPECIFIED,nullptr,nullptr,raw)!=SM_SUCCESS)
        throw std::runtime_error("create curve failed");
    return std::unique_ptr<SmBSplineCurve>(raw);
}

void point(const SmCurve &curve,double t) {
    SmPoint3d p;
    const auto status=curve.EvaluatePoint(t,p);
    std::cout<<"{\"status\":"<<status<<",\"point\":["<<p.x<<','<<p.y<<','<<p.z<<"]}";
}

int main() {
    SmApiCreateContext();std::cout<<std::setprecision(17)<<"{\"cases\":[";
    bool comma=false;
    for(auto scale : {1.,1e-6,1e6}) for(auto chart : {0,1,2}) {
        const double start=chart==0?0:chart==1?10:-8,end=start+1;
        auto a=curve(false,0,1,scale),b=curve(true,start,end,scale);
        CornerFillet worker(a.get(),b.get(),0.5*scale);
        SmBSplineCurve *raw=nullptr;
        const auto status=worker.DoFillet(raw);
        std::unique_ptr<SmBSplineCurve> result(raw);
        if(comma)std::cout<<',';comma=true;
        std::cout<<"{\"scale\":"<<scale<<",\"chart\":"<<chart<<",\"status\":"<<status;
        if(status==SM_SUCCESS&&result) {
            const auto ia=a->GetNaturalInterval(),ib=b->GetNaturalInterval(),f=result->GetNaturalInterval();
            std::cout<<",\"retained_domains\":[["<<ia.GetMin()<<','<<ia.GetMax()<<"],["<<ib.GetMin()<<','<<ib.GetMax()<<"]],\"fillet_samples\":[";
            point(*result,f.GetMin());std::cout<<',';point(*result,f.GetMid());std::cout<<',';point(*result,f.GetMax());
            std::cout<<"],\"first_start\":";point(*a,ia.GetMin());
            std::cout<<",\"second_end\":";point(*b,ib.GetMax());
        }
        std::cout<<'}';
    }
    std::cout<<"]}\n";
}
