#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <iomanip>
#include <iostream>

int main() {
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_CPOINT points[]={{0,0,0,1},{1,1,0,1},{2,0,0,1},{4,0,0,1},{5,-1,0,1},{6,0,0,1}};
    double knots[]={0,0,0,0.5,0.5,0.5,1,1,1};
    NL_CURVE source,whole,left,right,parts[2];
    N_CrvInitArrays(&source);N_CrvInitArrays(&whole);
    N_CrvInitArrays(&left);N_CrvInitArrays(&right);
    for(auto &part:parts) N_CrvInitArrays(&part);
    if(N_CrvFromCPtsAndKnots(&source,points,5,2,knots,8,&stack)!=NL_NO) return 2;
    const auto status=N_CrvElevateDegree(&source,2,&whole,&stack,&stack);
    if(N_CrvFromCPtsAndKnots(&left,points,2,2,knots,5,&stack)!=NL_NO
        || N_CrvFromCPtsAndKnots(&right,points+3,2,2,knots+3,5,&stack)!=NL_NO) return 3;
    const auto a=N_CrvElevateDegree(&left,2,&parts[0],&stack,&stack);
    const auto b=N_CrvElevateDegree(&right,2,&parts[1],&stack,&stack);
    if(a!=NL_NO || b!=NL_NO) return 4;
    std::cout<<std::setprecision(17)<<"{\"whole_curve_status\":"<<status
        <<",\"piece_statuses\":["<<a<<','<<b<<"],\"piece_controls\":[";
    for(int part=0;part<2;++part) {
        std::cout<<(part?",":"")<<'[';
        for(int i=0;i<=parts[part].pol->n;++i) {
            const auto &p=parts[part].pol->Pw[i];
            std::cout<<(i?",":"")<<'['<<p.x/p.w<<','<<p.y/p.w<<','<<p.z/p.w<<','<<p.w<<']';
        }
        std::cout<<']';
    }
    std::cout<<"],\"independent_join_limits\":[[2,0,0],[4,0,0]]}"<<std::endl;
}
