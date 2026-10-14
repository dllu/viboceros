#include <SmApiGeneral.h>
#include <SmBSplineSurface.h>
#include <SmBrep.h>
#include <SmCurve.h>
#include <SmEdge.h>
#include <SmEdgeuse.h>
#include <SmFace.h>
#include <SmSurface.h>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>

void check(SmStatus status, const char *operation) {
    if (status != SM_SUCCESS) throw std::runtime_error(std::string(operation)+" status "+std::to_string(status));
}

SmFace *patch(SmBrep &model, double y, std::array<double,2> domain) {
    SmTArray<SmPoint3d> points;
    for (double x : {0.,2.}) for (double v : {0.,1.}) points.Add(SmPoint3d(x,y+v,0.));
    SmTArray<double> u,v; u.Add(domain[0]);u.Add(domain[1]);v.Add(0.);v.Add(1.);
    SmTArray<ULONG> mu,mv;mu.Add(2);mu.Add(2);mv.Add(2);mv.Add(2);
    SmBSplineSurface *surface=nullptr;
    check(SmBSplineSurface::CreateCanonical(*SmApiGetOrCreateContext(),1,1,points,SM_SF_UNSPECIFIED,
        mu,mv,u,v,SM_KT_UNSPECIFIED,nullptr,nullptr,surface),"create patch");
    SmFace *face=nullptr;
    check(model.CreateFaceFromSurface(surface,surface->GetNaturalUVDomain(),face),"create face");
    return face;
}

SmEdgeuse *boundary(SmBrep &model, SmFace *face, double y) {
    SmTArray<SmEdgeuse*> uses;model.GetEdgeuses(uses);
    for (ULONG i=0;i<uses.GetSize();++i) {
        auto *use=uses[i];if (use->GetFace()!=face) continue;
        auto *edge=use->GetEdge();SmPoint3d point;
        check(edge->GetCurve()->EvaluatePoint(edge->GetInterval().Evaluate(0.5),point),"edge midpoint");
        if (std::abs(point[1]-y)<1e-12 && std::abs(point[0]-1.)<1e-12) return use;
    }
    throw std::runtime_error("missing boundary edge");
}

int main(int argc, char **argv) {
    try {
        SmApiCreateContext();std::cout<<std::setprecision(17);
        const double domains[4][2][2]={{{0.,1.},{0.,1.}},{{10.,30.},{10.,30.}},{{0.,1.},{2.,5.}},{{10.,30.},{2.,5.}}};
        const size_t selected_case=argc>1?std::stoul(argv[1]):0;
        const ULONG selected_flag=argc>2?std::stoul(argv[2]):2;
        const ULONG direction=argc>3?std::stoul(argv[3]):1;
        for (size_t case_id=selected_case;case_id<=selected_case;++case_id) for (ULONG flag : {selected_flag}) {
            if (case_id>=4 || flag>2) throw std::runtime_error("case must be 0..3 and flag 0..2");
            std::unique_ptr<SmBrep> model(new (*SmApiGetOrCreateContext()) SmBrep());
            model->SetTolerance(1e-10,FALSE);
            auto *first=patch(*model,0.,{domains[case_id][0][0],domains[case_id][0][1]});
            auto *second=patch(*model,2.,{domains[case_id][1][0],domains[case_id][1][1]});
            auto *a=boundary(*model,first,1.);auto *b=boundary(*model,second,2.);
            const auto ia=a->GetEdge()->GetInterval(),ib=b->GetEdge()->GetInterval();
            SmDerivSurfDefinition definition;
            definition.m_dStartTanLength=0.5;definition.m_dEndTanScale=1.;
            definition.m_eCurvatureType=flag==2?SM_CT_G2_FROM_SURFACE:SM_CT_NO_CURVATURE;
            SmSurface *raw=nullptr;
            const auto status=SmBrep::CreateEdgeEdgeBlend(*SmApiGetOrCreateContext(),flag,a,b,direction,&definition,raw);
            std::unique_ptr<SmSurface> output(raw);
            std::cout<<"{\"case\":"<<case_id<<",\"blend_flag\":"<<flag<<",\"direction_flag\":"<<direction<<",\"edge_domains\":[["<<ia.GetMin()<<','<<ia.GetMax()<<"],["<<ib.GetMin()<<','<<ib.GetMax()<<"]],\"status\":"<<status;
            if (status==SM_SUCCESS && output) {
                const auto domain=output->GetNaturalUVDomain();const auto lo=domain.GetMin(),hi=domain.GetMax();
                std::cout<<",\"output_domain\":[["<<lo[0]<<','<<hi[0]<<"],["<<lo[1]<<','<<hi[1]<<"]],\"samples\":[";
                double error=0.,boundary_error=0.,hermite_error=0.;bool finite=true;
                for (int i=0;i<=40;++i) for (int j=0;j<=40;++j) {
                    const double t=i/40.,v=j/40.;SmPoint3d point;
                    const auto eval=output->EvaluatePoint(SmPoint2d(lo[0]+t*(hi[0]-lo[0]),lo[1]+v*(hi[1]-lo[1])),point);
                    finite=finite && eval==SM_SUCCESS && std::isfinite(point[0]) && std::isfinite(point[1]) && std::isfinite(point[2]);
                    if (finite) error=std::max(error,std::hypot(std::hypot(point[0]-2*t,point[1]-(1+v)),point[2]));
                    const double along=flag==0?v:t,cross=flag==0?t:v;
                    const double target_y=flag==0?1+cross:flag==1?1+0.5*cross+1.5*cross*cross-cross*cross*cross:
                        1+0.5*cross+5*std::pow(cross,3)-7.5*std::pow(cross,4)+3*std::pow(cross,5);
                    if (finite && direction==1) hermite_error=std::max(hermite_error,std::hypot(std::hypot(point[0]-2*along,point[1]-target_y),point[2]));
                    if (finite && (cross==0. || cross==1.)) {
                        const double target_x=2*(direction==2 && cross==1.?1-along:along);
                        boundary_error=std::max(boundary_error,std::hypot(std::hypot(point[0]-target_x,point[1]-(1+cross)),point[2]));
                    }
                    if (i%10==0 && j%10==0) std::cout<<(i||j?",":"")<<"{\"u\":"<<t<<",\"v\":"<<v<<",\"status\":"<<eval<<",\"point\":["<<point[0]<<','<<point[1]<<','<<point[2]<<"]}";
                }
                std::cout<<"],\"all_finite\":"<<(finite?"true":"false")<<",\"distance_from_linear_parameter_model\":"<<error
                    <<",\"boundary_correspondence_error\":"<<boundary_error;
                if (direction==1) std::cout<<",\"independent_hermite_error\":"<<hermite_error;
            }
            std::cout<<"}\n"<<std::flush;
        }
    } catch (const std::exception &error) {std::cerr<<error.what()<<'\n';return 1;}
}
