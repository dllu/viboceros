// Retain native range endpoints for collinear paths with three or more visits.
#include <bridge.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <vector>
int main() {
    const std::vector<std::vector<double>> inputs={{0,3,1,4},{0,3,1,3},{0,2,1,3},{0,3,2,4},{0,3,1,2,0},{0,3,1,2,1},{0,3,1,4,2,5}};
    bool passed=true;
    std::cout<<std::setprecision(17)<<"{\"cases\":[";
    for(size_t kind=0;kind<inputs.size();++kind) {
        const auto &xs=inputs[kind];
        const double scale=*std::max_element(xs.begin(),xs.end());
        std::vector<double> points,knots={0,0};
        for(double x:xs)points.insert(points.end(),{x/scale,0,0,1});
        for(size_t i=1;i<xs.size()-1;++i)knots.push_back(double(i)/(xs.size()-1));
        knots.insert(knots.end(),{1,1});
        VbCurve *source=nullptr;VbCurveIntersections *events=nullptr;char error[1024]={};
        int status=vb_curve_create(1,points.data(),xs.size(),knots.data(),knots.size(),&source,error,sizeof(error));
        if(!status)status=vb_curve_self_intersect(source,1e-6/scale,&events,error,sizeof(error));
        std::vector<VbCurveIntersection> records(vb_curve_intersections_count(events));
        if(!status)status=vb_curve_intersections_copy(events,records.data(),records.size());
        if(kind)std::cout<<',';
        std::cout<<"{\"kind\":"<<kind<<",\"status\":"<<status<<",\"events\":[";
        double residual=0,image_error=0;
        for(size_t i=0;i<records.size();++i) {
            const auto &r=records[i];if(i)std::cout<<',';
            std::cout<<"{\"overlap\":"<<r.overlap<<",\"first\":["<<r.first[0]<<','<<r.first[1]
                <<"],\"second\":["<<r.second[0]<<','<<r.second[1]<<"]}";
            for(int end=0;end<(r.overlap?2:1);++end) {
                double a[3]={},b[3]={};
                if(vb_curve_evaluate(source,r.first[end],a,error,sizeof(error)) ||
                    vb_curve_evaluate(source,r.second[end],b,error,sizeof(error))) {status=2;continue;}
                const double gap=std::abs(a[0]-b[0])*scale;
                if(!std::isfinite(gap)) {status=2;continue;}
                residual=std::max(residual,gap);
            }
            if(r.overlap) {
                if(r.first[0]>r.second[0] || r.first[1]>r.second[1])status=2;
                auto image=[&](const double interval[2]) {
                    double minimum=scale,maximum=-scale;
                    std::vector<double> parameters={interval[0],interval[1]};
                    for(double t:knots)if(t>std::min(interval[0],interval[1]) && t<std::max(interval[0],interval[1]))parameters.push_back(t);
                    for(double t:parameters) {
                        double p[3]={};
                        if(vb_curve_evaluate(source,t,p,error,sizeof(error)) || !std::isfinite(p[0])) {status=2;continue;}
                        minimum=std::min(minimum,p[0]*scale);maximum=std::max(maximum,p[0]*scale);
                    }
                    return std::array<double,2>{minimum,maximum};
                };
                const auto a=image(r.first),b=image(r.second);
                image_error=std::max({image_error,std::abs(a[0]-b[0]),std::abs(a[1]-b[1])});
                if(r.first[0]==r.first[1] || r.second[0]==r.second[1])status=2;
            }
        }
        const bool valid=!status && residual<=1e-6 && image_error<=1e-6;passed=passed&&valid;
        std::cout<<"],\"endpoint_residual\":"<<residual<<",\"image_error\":"<<image_error<<",\"passed\":"<<(valid?"true":"false")<<'}';
        vb_curve_intersections_free(events);vb_curve_free(source);
    }
    std::cout<<"],\"passed\":"<<(passed?"true":"false")<<"}\n";
    return passed?0:1;
}
