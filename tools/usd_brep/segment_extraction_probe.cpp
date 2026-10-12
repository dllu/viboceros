#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <iomanip>
#include <iostream>

int main() {
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_CPOINT controls[]={{-1.5,0.75,0,0.75},{0,8,2,2},{2.5,-1,-0.5,0.5},{10,3.75,2.5,1.25}};
    double knots[]={-2,-1,0,0.8,2,3,4};
    NL_CURVE source,segment;
    N_CrvInitArrays(&source);N_CrvInitArrays(&segment);
    if(N_CrvFromCPtsAndKnots(&source,controls,3,2,knots,6,&stack)!=NL_NO) return 2;
    NL_CPOINT source_endpoint,actual;
    if(N_CrvEvalPt(&source,2,NL_LEFT,&source_endpoint)!=NL_NO) return 3;
    if(N_CrvExtractCrvSeg(&source,0.25,2,&segment,&stack,&stack)!=NL_NO) return 4;
    if(N_CrvEvalPt(&segment,2,NL_LEFT,&actual)!=NL_NO) return 5;
    std::cout<<std::setprecision(17)
             // Independent de Boor endpoint limit, also retained in the Rhino
             // trim-rational-active-end capture. Native endpoint evaluation
             // itself assumes clamped padding and cannot serve as the oracle.
             <<"{\"expected_endpoint\":[7.25,1.75,1.25],\"unchecked_source_endpoint\":["
             <<source_endpoint.x/source_endpoint.w<<','<<source_endpoint.y/source_endpoint.w<<','<<source_endpoint.z/source_endpoint.w
             <<"],\"unchecked_segment_endpoint\":["<<actual.x/actual.w<<','<<actual.y/actual.w<<','<<actual.z/actual.w<<"]}\n";
}
