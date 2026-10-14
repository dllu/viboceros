// Qualify native point/range curve intersections before adapter integration.
#include <SmApiGeneral.h>
#include <SmBSplineCurve.h>
#include <SmSolutionArray.h>
#include <SmCurveCache.h>
#include <SmCacheMgr.h>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>

std::unique_ptr<SmBSplineCurve> curve(int degree,std::initializer_list<SmPoint3d> controls) {
    SmTArray<SmPoint3d> points;for(auto p:controls)points.Add(p);
    SmTArray<double> knots;knots.Add(0);knots.Add(1);
    SmTArray<ULONG> multiplicities;multiplicities.Add(degree+1);multiplicities.Add(degree+1);
    SmBSplineCurve *raw=nullptr;
    const auto status=SmBSplineCurve::CreateCanonical(*SmApiGetOrCreateContext(),3,degree,points,
        SM_CF_UNSPECIFIED,multiplicities,knots,SM_KT_UNSPECIFIED,nullptr,nullptr,raw);
    if(status!=SM_SUCCESS)throw std::runtime_error("create intersection input");
    return std::unique_ptr<SmBSplineCurve>(raw);
}
int main() {
    SmApiCreateContext();std::cout<<std::setprecision(17)<<"{\"cases\":[";
    bool comma=false;
    for(int kind=0;kind<12;++kind) for(int skip=0;skip<2;++skip) {
        const double c=0.3,k=kind>=9?0.0625:1.;
        const double origin=kind==10?k*c*c:0,scale=kind==10?std::hypot(1.,k*(1-c)*(1-c)-origin):1.;
        auto a=kind>=8?curve(2,{{0,(k*c*c-origin)/scale,0},{0.5/scale,(k*(c*c-c)-origin)/scale,0},{1/scale,(k*(1-c)*(1-c)-origin)/scale,0}}):
            kind==1||kind==2||kind>=6?curve(2,{{0,0,0},{1,2,0},{2,0,0}}):curve(1,{{0,0,0},{2,0,0}});
        auto b=kind==0?curve(1,{{1,-1,0},{1,1,0}}):
            kind==1?curve(1,{{0,1,0},{2,1,0}}):
            kind==2?curve(1,{{0,0.5,0},{2,0.5,0}}):
            kind==3?curve(1,{{0.5,0,0},{3,0,0}}):
            kind==4?curve(1,{{3,0,0},{0.5,0,0}}):
            kind==6?curve(1,{{0,1-1e-10,0},{2,1-1e-10,0}}):
            kind==7?curve(1,{{0,1+1e-10,0},{2,1+1e-10,0}}):
            kind>=8?curve(1,{{0,-origin/scale,0},{1/scale,-origin/scale,0}}):curve(1,{{0,2e-8,0},{2,2e-8,0}});
        if(kind==11) {
            a=curve(2,{{0,0,0},{0.4998438232040613,-0.0187441433701523,0},{0.9996876464081226,0.02499219116020306,0}});
            b=curve(1,{{0,-0.0056232430110456895,0},{0.9996876464081226,-0.0056232430110456895,0}});
        }
        SmSolutionArray solutions;
        const auto status=a->GlobalCurveIntersect(a->GetNaturalInterval(),*b,b->GetNaturalInterval(),kind==6||kind==7?1e-12:1e-9,solutions,skip?TRUE:FALSE);
        if(comma)std::cout<<',';comma=true;
        std::cout<<"{\"kind\":"<<kind<<",\"skip_coincidence\":"<<skip<<",\"status\":"<<status<<",\"solutions\":[";
        for(ULONG i=0;i<solutions.GetSize();++i) {
            const auto &s=solutions[i];
            if(i)std::cout<<',';
            SmPoint3d p,q;a->EvaluatePoint(s.m_vStart[0],p);b->EvaluatePoint(s.m_vStart[1],q);
            std::cout<<"{\"type\":"<<s.m_eSolutionType<<",\"start\":["<<s.m_vStart[0]<<','<<s.m_vStart[1]
                <<"],\"end\":["<<s.m_vEnd[0]<<','<<s.m_vEnd[1]<<"],\"point\":["<<p.x<<','<<p.y<<','<<p.z
                <<"],\"residual\":"<<p.DistanceBetween(q)<<'}';
        }
        std::cout<<"]";
        if(kind>=8) {
            const auto *analytic=static_cast<SmCurveCache*>(SmCacheMgr::GetOrCreateObjectCache(SM_OC_CURVE,a.get()))->GetAnalyticCurve();
            std::cout<<",\"cached_analytic\":\""<<(analytic?analytic->GetTypeString():"None")<<'"';
        }
        if((kind==6||kind>=9) && skip==0) {
            std::cout<<",\"local_seeds\":[";
            bool local_comma=false;
            for(double guess:{0.,0.25,kind==6?0.49:0.29,kind==6?0.49999:0.29999,kind==6?0.5:0.3,kind==6?0.50001:0.30001,kind==6?0.51:0.31,0.75,1.}) {
                SmBoolean found=FALSE;double x=0,y=0,gap=0;
                const auto local=a->LocalCurveIntersect(a->GetNaturalInterval(),*b,b->GetNaturalInterval(),
                    1e-12,guess,guess,found,x,y,gap);
                if(local_comma)std::cout<<',';local_comma=true;
                std::cout<<"{\"guess\":"<<guess<<",\"status\":"<<local<<",\"found\":"<<(found?"true":"false")
                    <<",\"parameters\":["<<x<<','<<y<<"],\"gap\":"<<gap<<'}';
            }
            std::cout<<"]";
        }
        std::cout<<'}';
    }
    std::cout<<"]}\n";
}
