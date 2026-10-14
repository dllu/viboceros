// Direct upstream surface intersector: coupled XYZ/UV curves and classifications.
#include <brep_data.h>
#include <SmApiGeneral.h>
#include <SmSurface.h>
#include <SmCurve.h>
#include <SmTol.h>
#include <array>
#include <algorithm>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
#include <vector>

struct Curves {
    SmTArray<SmCurve*> values;
    ~Curves() { for(ULONG i=0;i<values.GetSize();++i)delete values[i]; }
};
std::unique_ptr<SmSurface> patch(const std::array<std::array<double,3>,4> &points) {
    std::vector<double> controls;
    for(const auto &p:points)controls.insert(controls.end(),{p[0],p[1],p[2],1});
    const double knots[]={0,0,1,1};
    const VbNurbsData data={{1,1},{2,2},{4,4},{knots,knots},controls.data()};
    return std::unique_ptr<SmSurface>(vb_import_query_surface(data));
}
int main() {
    SmApiCreateContext();std::cout<<std::unitbuf<<std::setprecision(17)<<"{\"cases\":[";
    for(int kind=0;kind<6;++kind) {
        auto a=patch({{{0,0,0},{1,0,0},{0,1,0},{1,1,0}}});
        std::unique_ptr<SmSurface> b;
        if(kind==0)b=patch({{{0.5,0,-1},{0.5,1,-1},{0.5,0,1},{0.5,1,1}}});
        else if(kind==1)b=patch({{{0,0,1},{1,0,1},{0,1,1},{1,1,1}}});
        else if(kind==2)b=patch({{{0.25,0.25,0},{1.25,0.25,0},{0.25,1.25,0},{1.25,1.25,0}}});
        else if(kind==3)b=patch({{{1,1,0},{2,1,0},{1,2,0},{2,2,0}}});
        else if(kind==4)b=patch({{{0,0,0.25},{1,0,-0.25},{0,1,-0.25},{1,1,0.25}}});
        else {
            a=patch({{{0,0,0.25},{1,0,0.25},{0,1,0.25},{1,1,0.25}}});
            b=patch({{{0,0,0},{1,0,0},{0,1,0},{1,1,1}}});
        }
        Curves xyz,uv1,uv2;SmTArray<SmTsectCurveType> types;SmTArray<double> deviations;
        const SmBoolean edges[]={TRUE,TRUE};const SmApproxTol3d tolerance(1e-9);const double angle=0.1;
        const auto status=a->GlobalSurfaceIntersect(*SmApiGetOrCreateContext(),a->GetNaturalUVDomain(),*b,b->GetNaturalUVDomain(),
            edges,&tolerance,&angle,&xyz.values,&uv1.values,&uv2.values,&types,&deviations);
        if(kind)std::cout<<',';
        std::cout<<"{\"kind\":"<<kind<<",\"status\":"<<status<<",\"counts\":["<<xyz.values.GetSize()<<','<<uv1.values.GetSize()<<','<<uv2.values.GetSize()
            <<','<<types.GetSize()<<','<<deviations.GetSize()<<"],\"curves\":[";
        for(ULONG i=0;i<xyz.values.GetSize();++i) {
            if(i)std::cout<<',';
            if(!xyz.values[i]) {std::cout<<"{\"type\":"<<(i<types.GetSize()?int(types[i]):-1)<<",\"missing_xyz\":true}";continue;}
            const auto interval=xyz.values[i]->GetNaturalInterval();
            double gap=0;int evaluate_status=1000;
            for(int station=0;station<=64;++station) {
                const double fraction=station/64.;const double t=interval.GetMin()*(1-fraction)+interval.GetMax()*fraction;
                SmPoint3d p,s1,s2,q1,q2;
                evaluate_status=xyz.values[i]->EvaluatePoint(t,p);
                if(evaluate_status!=SM_SUCCESS)break;
                if(i>=uv1.values.GetSize() || i>=uv2.values.GetSize() || !uv1.values[i] || !uv2.values[i]) {evaluate_status=-1;break;}
                const auto d1=uv1.values[i]->GetNaturalInterval(),d2=uv2.values[i]->GetNaturalInterval();
                evaluate_status=uv1.values[i]->EvaluatePoint(d1.GetMin()*(1-fraction)+d1.GetMax()*fraction,q1);
                if(evaluate_status!=SM_SUCCESS)break;
                evaluate_status=uv2.values[i]->EvaluatePoint(d2.GetMin()*(1-fraction)+d2.GetMax()*fraction,q2);
                if(evaluate_status!=SM_SUCCESS)break;
                evaluate_status=a->EvaluatePoint(SmPoint2d(q1.x,q1.y),s1);
                if(evaluate_status!=SM_SUCCESS)break;
                evaluate_status=b->EvaluatePoint(SmPoint2d(q2.x,q2.y),s2);
                if(evaluate_status!=SM_SUCCESS)break;
                gap=std::max({gap,p.DistanceBetween(s1),p.DistanceBetween(s2)});
            }
            SmPoint3d first,last;xyz.values[i]->EvaluatePoint(interval.GetMin(),first);xyz.values[i]->EvaluatePoint(interval.GetMax(),last);
            std::cout<<"{\"type\":"<<(i<types.GetSize()?int(types[i]):-1)<<",\"evaluate_status\":"<<evaluate_status<<",\"gap\":"<<gap
                <<",\"start\":["<<first.x<<','<<first.y<<','<<first.z<<"],\"end\":["<<last.x<<','<<last.y<<','<<last.z<<"]}";
        }
        std::cout<<"]}";
    }
    std::cout<<"]}\n";
}
