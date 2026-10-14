// Reproduce normal-drop interval starts through public usd-brep surface APIs.
#include <SmApiGeneral.h>
#include <SmApiCurves.h>
#include <SmApiSurfaces.h>
#include <SmBSplineCurve.h>
#include <SmSurface.h>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
struct Curves {
    SmTArray<SmBSplineCurve*> values;
    ~Curves(){for(ULONG i=0;i<values.GetSize();++i)delete values[i];}
};
int main() {
    SmApiCreateContext();
    SmSurface *raw=nullptr;
    if(SmApiCreateSurfaceFromCornerPoints(SmPoint3d(0,0,0),SmPoint3d(10,0,0),SmPoint3d(0,10,0),SmPoint3d(10,10,0),raw)!=SM_SUCCESS || !raw)return 2;
    std::unique_ptr<SmSurface> surface(raw);
    std::cout<<std::setprecision(17);
    const double xs[][2]={{1,9},{-2,12},{1,12},{-2,9},{-2,-1},{11,12}};
    for(int height=0;height<=1;++height)for(int c=0;c<6;++c) {
        SmTArray<SmPoint3d> points;points.Add(SmPoint3d(xs[c][0],5,height));points.Add(SmPoint3d((2*xs[c][0]+xs[c][1])/3.,5,height));points.Add(SmPoint3d((xs[c][0]+2*xs[c][1])/3.,5,height));points.Add(SmPoint3d(xs[c][1],5,height));
        SmBSplineCurve *source_raw=nullptr;
        if(SmApiCreateCurve(points,source_raw)!=SM_SUCCESS || !source_raw)return 3;
        std::unique_ptr<SmBSplineCurve> source(source_raw);
        Curves curves;double drop=0,gap=0;
        const auto status=surface->DropAndTrimCurve(*SmApiGetOrCreateContext(),surface->GetNaturalUVDomain(),
            *source,source->GetNaturalInterval(),1e-10,drop,gap,curves.values,TRUE,height==0);
        std::cout<<"{\"case\":"<<c<<",\"height\":"<<height<<",\"status\":"<<status<<",\"curve_count\":"<<curves.values.GetSize()<<",\"curves\":[";
        for(ULONG i=0;i<curves.values.GetSize();++i) {
            if(i)std::cout<<',';
            std::cout<<'[';
            for(int j=0;j<=16;++j) {
                SmPoint3d uv,point;
                const auto interval=curves.values[i]->GetNaturalInterval();
                if(curves.values[i]->EvaluatePoint(interval.Evaluate(j/16.),uv)!=SM_SUCCESS ||
                    surface->EvaluatePoint(SmPoint2d(uv.x,uv.y),point)!=SM_SUCCESS ||
                    !std::isfinite(point.x) || !std::isfinite(point.y) || !std::isfinite(point.z))return 4;
                std::cout<<(j?",":"")<<'['<<point.x<<','<<point.y<<','<<point.z<<']';
            }
            std::cout<<']';
        }
        std::cout<<"]}\n"<<std::flush;
    }
}
