// Exercise the actual contact C ABI in a unit-radius working frame.
#include <bridge.h>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <vector>

std::array<double,3> rotate(double x,double y,int axis) {
    if(axis==1)return {0,x,y};
    if(axis==2)return {y,0,x};
    return {x,y,0};
}
double error(const double *a,const double *b) {
    return std::hypot(std::hypot(a[0]-b[0],a[1]-b[1]),a[2]-b[2]);
}

int main() {
    bool passed=true,comma=false;
    std::cout<<std::setprecision(17)<<"{\"cases\":[";
    for(int kind=0;kind<4;++kind) for(int chart=0;chart<3;++chart) for(int axis=0;axis<3;++axis) {
        const bool curved=kind==1||kind==2;
        const double lo=chart==0?0:chart==1?-4:10,hi=lo+(chart?11:1);
        const double start=chart==0?0:chart==1?10:-8,end=start+(chart?6:1);
        double ka[]={lo,lo,lo,hi,hi,hi};
        double kb[]={start,start,start,end,end,end};
        const double kl[]={start,start,end,end};
        std::vector<double> a,b;
        const std::array<std::array<double,2>,3> controls=kind==3
            ?std::array<std::array<double,2>,3>{{{-20,5},{0,-7.5},{0,0}}}
            :std::array<std::array<double,2>,3>{{{-4,-4},{0,-4},{0,0}}};
        for(auto p : controls) {
            const auto xyz=rotate(p[0],p[1],axis);a.insert(a.end(),{xyz[0],xyz[1],xyz[2],1.});
        }
        for(int i=0;i<(curved?3:2);++i) {
            const auto xyz=rotate(i?(kind==3?-20:4):0,i==2?4:0,axis);
            b.insert(b.end(),{xyz[0],xyz[1],xyz[2],kind==1&&i==1?std::sqrt(0.5):1.});
        }
        VbCurve *first=nullptr,*second=nullptr;char message[1024]={};
        int status=vb_curve_create(2,a.data(),3,ka,6,&first,message,sizeof(message));
        if(!status)status=vb_curve_create(curved?2:1,b.data(),curved?3:2,curved?kb:kl,curved?6:4,&second,message,sizeof(message));
        double contacts[2]={},points[9]={};
        if(!status)status=vb_curve_fillet_contacts(first,second,1.,1e-13,contacts,points,message,sizeof(message));
        double residual=0,source_error=0;
        if(!status) {
            double q[3];
            status=vb_curve_evaluate(first,contacts[0],q,message,sizeof(message));
            residual=error(q,points);
            if(!status)status=vb_curve_evaluate(second,contacts[1],q,message,sizeof(message));
            residual=std::max(residual,error(q,points+6));
            if(!status)status=vb_curve_evaluate(first,lo,q,message,sizeof(message));
            const auto original=rotate(controls[0][0],controls[0][1],axis);source_error=error(q,original.data());
            if(!status)status=vb_curve_evaluate(second,end,q,message,sizeof(message));
            const auto other=rotate(kind==3?-20:4,curved?4:0,axis);source_error=std::max(source_error,error(q,other.data()));
            if(!(contacts[0]>lo&&contacts[0]<hi&&contacts[1]>start&&contacts[1]<end))status=2;
        }
        const bool valid=status==0&&residual<1e-10&&source_error<1e-12;
        passed=passed&&valid;
        if(comma)std::cout<<',';comma=true;
        std::cout<<"{\"kind\":"<<kind<<",\"chart\":"<<chart<<",\"axis\":"<<axis<<",\"status\":"<<status
            <<",\"point_residual\":"<<residual<<",\"source_endpoint_error\":"<<source_error<<",\"contacts\":["<<contacts[0]<<','<<contacts[1]<<"]"
            <<",\"passed\":"<<(valid?"true":"false")<<'}';
        vb_curve_free(first);vb_curve_free(second);
    }
    double p[2]={},samples[9]={};char error_message[1024]={};
    const bool invalid=vb_curve_fillet_contacts(nullptr,nullptr,1.,1e-13,p,samples,error_message,sizeof(error_message))!=0;
    passed=passed&&invalid;
    std::cout<<"],\"invalid_input_rejected\":"<<(invalid?"true":"false")<<",\"passed\":"<<(passed?"true":"false")<<"}\n";
    return passed?0:1;
}
