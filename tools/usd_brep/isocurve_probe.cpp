#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <iomanip>
#include <iostream>

int main() {
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    double u[]={0,0,1,1},v[]={-2,-1,0,0.8,2,3,4};
    NL_CPOINT first[]={{0,0,0,1},{0,1,0,1},{0,2,0,1},{0,3,0,1}};
    NL_CPOINT second[]={{1,0,0,1},{1,1,0,1},{1,2,0,1},{1,3,0,1}};
    NL_CPOINT *points[]={first,second};
    NL_SURFACE source;
    NL_CURVE curve;
    N_SrfInitArrays(&source);N_CrvInitArrays(&curve);
    if(N_SrfFromCPtsAndKnots(&source,points,1,3,1,2,u,v,3,6,&stack)!=NL_NO) return 2;
    const auto end_status=N_SrfExtractIsoCrv(&source,2,NL_UDIR,&curve,&stack);
    if(end_status!=NL_NO) return 3;
    const double actual=curve.pol->Pw[0].y/curve.pol->Pw[0].w;
    double breaks[]={0,0,0,0.5,0.5,0.5,1,1,1};
    NL_CPOINT left[]={{0,0,0,1},{0,1,0,1},{0,2,0,1},{0,3,0,1},{0,4,0,1},{0,5,0,1}};
    NL_CPOINT right[]={{1,0,0,1},{1,1,0,1},{1,2,0,1},{1,3,0,1},{1,4,0,1},{1,5,0,1}};
    NL_CPOINT *rows[]={left,right};
    NL_SURFACE split;
    NL_CURVE boundary;
    N_SrfInitArrays(&split);N_CrvInitArrays(&boundary);
    if(N_SrfFromCPtsAndKnots(&split,rows,1,5,1,2,u,breaks,3,8,&stack)!=NL_NO) return 4;
    const auto break_status=N_SrfExtractIsoCrv(&split,0.5,NL_UDIR,&boundary,&stack);
    // Independent endpoint de Boor limit: y=2+6/11=28/11. At the full-order
    // interior break the right-limit row is y=3; no native evaluator is used
    // to validate either observation.
    std::cout<<std::setprecision(17)<<"{\"expected_endpoint_y\":"<<28.0/11.0
        <<",\"unchecked_endpoint_y\":"<<actual
        <<",\"full_order_expected_y\":3,\"full_order_native_failed\":"
        <<(break_status!=NL_NO?"true":"false")<<"}\n";
}
