#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <iostream>
#include <iomanip>
int main() {
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_CPOINT points[]={{0,0,0,1},{2,4,1,1},{5,-1,2,1},{7,3,-1,1},{9,1,0,1},{12,5,2,1},{15,-2,1,1}};
    double knots[]={0,0,0,0,1,3,5,8,8,8,8};
    NL_CURVE source,result;
    N_CrvInitArrays(&source);N_CrvInitArrays(&result);
    if(N_CrvFromCPtsAndKnots(&source,points,6,3,knots,10,&stack)!=NL_NO) return 2;
    NL_INDEX removed=0;
    if(N_CrvRemoveKnot(&source,3,1,1e10,&removed,&result,&stack)!=NL_NO) return 3;
    if(removed!=1) return 4;
    std::cout<<std::setprecision(17)<<"{\"degree\":"<<result.p<<",\"control_points\":[";
    for(NL_INDEX i=0;i<=result.pol->n;++i) {
        const auto p=result.pol->Pw[i];
        if(i)std::cout<<',';
        std::cout<<"{\"point\":["<<p.x/p.w<<','<<p.y/p.w<<','<<p.z/p.w<<"],\"weight\":"<<p.w<<'}';
    }
    std::cout<<"],\"knots\":[";
    for(NL_INDEX i=0;i<=result.knt->m;++i) {if(i)std::cout<<',';std::cout<<result.knt->U[i];}
    std::cout<<"]}\n";
}
