#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <iomanip>
#include <iostream>

int main() {
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_CPOINT first[]={{0,0,0,1},{0,1,0,1}},second[]={{1,0,0,1},{1,1,0,1}};
    NL_CPOINT *controls[]={first,second};
    double knots[]={0,0,1,1};
    NL_SURFACE source,patch;
    N_SrfInitArrays(&source);N_SrfInitArrays(&patch);
    if(N_SrfFromCPtsAndKnots(&source,controls,1,1,1,1,knots,knots,3,3,&stack)!=NL_NO) return 2;
    const double requested=1e-9;
    if(N_SrfExtractPatch(&source,requested,0.75,0,1,&patch,&stack,&stack)!=NL_NO) return 3;
    // The bilinear source is independently S(u,v)=(u,v,0). All extracted
    // left-boundary controls must therefore have x=requested. Reading controls
    // avoids using the native endpoint evaluator as its own oracle.
    NL_INDEX n,m;
    NL_CPOINT **points;
    N_SrfGetCPts(&patch,&n,&m,&points);
    const auto &actual=points[0][0];
    std::cout<<std::setprecision(17)<<"{\"requested_start\":"<<requested
        <<",\"reported_start\":"<<patch.knu->U[patch.p]
        <<",\"expected_boundary_x\":"<<requested
        <<",\"unchecked_boundary_x\":"<<actual.x/actual.w<<"}\n";
}
