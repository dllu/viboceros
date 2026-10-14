// Actual C ABI, ownership/export and independent analytic offset checks.
#include <bridge.h>
#include <array>
#include <algorithm>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <limits>
#include <vector>

std::array<double,3> rotate(double x,double y,int plane) {
    if(plane==1)return {0,x,y};
    if(plane==2)return {y,0,x};
    return {x,y,0};
}
double error(const double *a,const double *b) {
    return std::hypot(std::hypot(a[0]-b[0],a[1]-b[1]),a[2]-b[2]);
}
// Independently evaluate the exported homogeneous control net with de Boor.
std::array<double,3> evaluate(size_t degree,const std::vector<double>& points,
                            const std::vector<double>& knots,double t) {
    const size_t count=points.size()/4;
    const size_t span=t>=knots[count]?count-1:std::upper_bound(knots.begin()+degree,
        knots.begin()+count+1,t)-knots.begin()-1;
    std::vector<std::array<double,4>> work(degree+1);
    for(size_t j=0;j<=degree;++j) {
        const auto i=span-degree+j;
        for(size_t k=0;k<3;++k)work[j][k]=points[4*i+k]*points[4*i+3];
        work[j][3]=points[4*i+3];
    }
    for(size_t r=1;r<=degree;++r) for(size_t j=degree;j>=r;--j) {
        const size_t i=span-degree+j;
        const double alpha=(t-knots[i])/(knots[i+degree-r+1]-knots[i]);
        for(size_t k=0;k<4;++k)work[j][k]=(1-alpha)*work[j-1][k]+alpha*work[j][k];
    }
    return {work[degree][0]/work[degree][3],work[degree][1]/work[degree][3],work[degree][2]/work[degree][3]};
}
int main() {
    bool passed=true,comma=false;
    std::cout<<std::setprecision(17)<<"{\"cases\":[";
    for(int kind=0;kind<2;++kind) for(int chart=0;chart<2;++chart)
    for(int plane=0;plane<3;++plane) for(double distance:{-0.125,0.125})
    for(double tolerance:{1e-5,1e-9}) {
        const double lo=chart?-4:0,hi=chart?7:1,w=std::sqrt(0.5);
        const std::array<std::array<double,2>,3> xy=kind
            ?std::array<std::array<double,2>,3>{{{1,0},{1,1},{0,1}}}
            :std::array<std::array<double,2>,3>{{{0,0},{1,2},{2,0}}};
        std::vector<double> controls;
        for(size_t i=0;i<3;++i) {
            const auto p=rotate(xy[i][0],xy[i][1],plane);
            controls.insert(controls.end(),{p[0],p[1],p[2],kind&&i==1?w:1.});
        }
        const double knots[]={lo,lo,lo,hi,hi,hi};
        const std::array<double,3> n=plane==1?std::array<double,3>{1,0,0}:
            plane==2?std::array<double,3>{0,1,0}:std::array<double,3>{0,0,1};
        VbCurve *source=nullptr,*result=nullptr;char message[1024]={};
        int status=vb_curve_create(2,controls.data(),3,knots,6,&source,message,sizeof(message));
        if(!status)status=vb_curve_offset_approx(source,n.data(),distance,tolerance,&result,message,sizeof(message));
        size_t degree=0,count=0,knot_count=0;double max_error=0,native_error=0;
        std::vector<double> output,output_knots;
        if(!status)status=vb_curve_sizes(result,&degree,&count,&knot_count);
        if(!status) {
            output.resize(4*count);output_knots.resize(knot_count);
            status=vb_curve_copy(result,output.data(),count,output_knots.data(),knot_count);
        }
        if(!status) for(int i=0;i<=1024;++i) {
            const double u=i/1024.,t=lo+(hi-lo)*u,q=1-u,dy=4-8*u,speed=std::hypot(2.,dy);
            const double den=q*q+2*w*q*u+u*u;
            const auto expected=kind?rotate((1-distance)*(q*q+2*w*q*u)/den,
                (1-distance)*(2*w*q*u+u*u)/den,plane):
                rotate(2*u-distance*dy/speed,4*u*q+distance*2/speed,plane);
            const auto independent=evaluate(degree,output,output_knots,t);
            const double point_error=error(independent.data(),expected.data());
            if(!std::isfinite(point_error)) {status=2;break;}
            max_error=std::max(max_error,point_error);
            double p[3];status=vb_curve_evaluate(result,t,p,message,sizeof(message));
            if(status)break;
            const double export_error=error(p,independent.data());
            if(!std::isfinite(export_error)) {status=2;break;}
            native_error=std::max(native_error,export_error);
        }
        size_t source_degree=0,source_count=0,source_knots=0;
        std::vector<double> original(12),original_knots(6);
        const bool unchanged=source && vb_curve_sizes(source,&source_degree,&source_count,&source_knots)==0
            && source_degree==2 && source_count==3 && source_knots==6
            && vb_curve_copy(source,original.data(),3,original_knots.data(),6)==0
            && original==controls && std::equal(original_knots.begin(),original_knots.end(),knots);
        const bool valid=status==0 && max_error<=tolerance && native_error<=1e-12 && unchanged;
        passed=passed&&valid;
        if(comma)std::cout<<',';comma=true;
        std::cout<<"{\"kind\":"<<kind<<",\"chart\":"<<chart<<",\"plane\":"<<plane
            <<",\"distance\":"<<distance<<",\"tolerance\":"<<tolerance<<",\"status\":"<<status
            <<",\"point_error\":"<<max_error<<",\"native_export_error\":"<<native_error
            <<",\"controls\":"<<count<<",\"source_unchanged\":"<<(unchanged?"true":"false")
            <<",\"passed\":"<<(valid?"true":"false")<<'}';
        vb_curve_free(source);vb_curve_free(result);
    }
    VbCurve *out=nullptr;char message[1024]={};const double n[]={0,0,1};
    const bool rejected=vb_curve_offset_approx(nullptr,n,0.125,1e-6,&out,message,sizeof(message))!=0 && !out;
    const double controls[]={0,0,0,1,1,2,0,1,2,0,0,1},knots[]={0,0,0,1,1,1},zero[]={0,0,0};
    VbCurve *source=nullptr;
    const bool created=vb_curve_create(2,controls,3,knots,6,&source,message,sizeof(message))==0;
    bool invalid=created;
    if(created) {
        invalid=vb_curve_offset_approx(source,zero,0.125,1e-6,&out,message,sizeof(message))!=0 && !out;
        invalid=invalid && vb_curve_offset_approx(source,n,std::numeric_limits<double>::quiet_NaN(),1e-6,&out,message,sizeof(message))!=0 && !out;
        invalid=invalid && vb_curve_offset_approx(source,n,0.125,-1.,&out,message,sizeof(message))!=0 && !out;
    }
    const bool bounded=created && vb_curve_offset_approx(source,n,0.125,1e-30,&out,message,sizeof(message))!=0 && !out;
    vb_curve_free(source);vb_curve_free(out);
    passed=passed&&rejected&&invalid&&bounded;
    std::cout<<"],\"invalid_input_rejected\":"<<(rejected?"true":"false")
        <<",\"invalid_values_rejected\":"<<(invalid?"true":"false")
        <<",\"extreme_tolerance_rejected\":"<<(bounded?"true":"false")
        <<",\"passed\":"<<(passed?"true":"false")<<"}\n";
    return passed?0:1;
}
