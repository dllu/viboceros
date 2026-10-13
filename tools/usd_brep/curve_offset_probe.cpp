#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>

std::array<double,3> evaluate(const NL_CURVE &curve,double t) {
    const int p=curve.p,n=curve.pol->n;
    const auto *u=curve.knt->U;
    const int span=t==u[n+1]?n:std::upper_bound(u,u+curve.knt->m+1,t)-u-1;
    std::array<std::array<double,4>,NL_MAXDEG+1> q;
    for(int j=0;j<=p;++j) {
        const auto c=curve.pol->Pw[span-p+j];const double w=N_IsCrvRat(const_cast<NL_CURVE *>(&curve))?c.w:1.;q[j]={c.x,c.y,c.z,w};
    }
    for(int r=1;r<=p;++r) for(int j=p;j>=r;--j) {
        const int i=span-p+j;const double a=(t-u[i])/(u[i+p-r+1]-u[i]);
        for(int k=0;k<4;++k) q[j][k]=(1-a)*q[j-1][k]+a*q[j][k];
    }
    return {q[p][0]/q[p][3],q[p][1]/q[p][3],q[p][2]/q[p][3]};
}
int main() {
    NL_STACKS stack;SmStackHandler cleanup(&stack);
    NL_CPOINT points[]={{0,0,0,1},{1,2,0,1},{2,0,0,1}};
    double knots[]={0,0,0,1,1,1};
    NL_CURVE source,result;N_CrvInitArrays(&source);N_CrvInitArrays(&result);
    if(N_CrvFromCPtsAndKnots(&source,points,2,2,knots,5,&stack)!=NL_NO) return 2;
    NL_VECTOR normal={0,0,1};const double distance=0.125,tolerance=1e-5;
    const auto status=N_CrvOffsetPtSampling(&source,normal,distance,3,NL_YES,NL_INHERITED,tolerance,&result,&stack);
    std::cout<<std::setprecision(17)<<"{\"status\":"<<status;
    if(status==NL_NO) {
        double maximum=0;for(int i=0;i<=1000;++i) {
            const double t=i/1000.,dy=4-8*t,speed=std::hypot(2.,dy);const auto q=evaluate(result,t);
            maximum=std::max(maximum,std::hypot(q[0]-(2*t-distance*dy/speed),q[1]-(4*t*(1-t)+2*distance/speed)));
        }
        std::cout<<",\"degree\":"<<result.p<<",\"controls\":"<<result.pol->n+1
            <<",\"domain\":["<<result.knt->U[result.p]<<','<<result.knt->U[result.pol->n+1]
            <<"],\"tolerance\":"<<tolerance<<",\"independent_max_error\":"<<maximum;
        std::cout<<",\"samples\":[";
        for(int i=0;i<5;++i) {const double t=i/4.;const auto q=evaluate(result,t);NL_CPOINT native;N_CrvEvalPt(&result,t,NL_EPOINT,&native);
            std::cout<<(i?",":"")<<"{\"t\":"<<t<<",\"independent_evaluator\":["<<q[0]<<','<<q[1]<<','<<q[2]<<"],\"native_evaluator\":["<<native.x<<','<<native.y<<','<<native.z<<"]}";
        }
        std::cout<<"]";
    }
    std::cout<<",\"circle_cases\":[";
    for(int sign : {1,-1}) {
        NL_CURVE circle,offset;N_CrvInitArrays(&circle);N_CrvInitArrays(&offset);
        NL_POINT center={0,0,0};NL_VECTOR x={1,0,0},y={0,1,0},n={0,0,static_cast<double>(sign)};
        if(N_CreateCircArc(center,x,y,2.,0.,360.,NL_QUADRATIC,&circle,&stack)!=NL_NO) return 3;
        const auto status=N_CrvOffsetPtSampling(&circle,n,distance,2,NL_NO,NL_INHERITED,tolerance,&offset,&stack);
        std::cout<<(sign==1?"":",")<<"{\"normal_z\":"<<sign<<",\"status\":"<<status<<",\"expected_radius\":"<<2-sign*distance;
        if(status==NL_NO) {const auto p=evaluate(offset,offset.knt->U[offset.p]);std::cout<<",\"actual_radius\":"<<std::hypot(p[0],p[1]);}
        std::cout<<"}";
    }
    std::cout<<"]}\n";return 0;
}
