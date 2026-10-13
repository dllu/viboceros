#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <iomanip>
#include <iostream>

int main() {
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    // The exact target is X(u)=u on [0,2]. Linear precision gives the
    // independent control coordinates [-0.5,0.4,1.4,2.5] in this basis.
    double knots[]={-2,-1,0,0.8,2,3,4},parameters[]={0,0.4,1.4,2};
    NL_CPOINT targets[]={{0,0,0,1},{0.4,0,0,1},{1.4,0,0,1},{2,0,0,1}};
    NL_KNOTVECTOR basis={6,knots};
    NL_CURVE curve;
    N_CrvInitArrays(&curve);
    const auto status=N_FitCrvInterpGivenParams(targets,3,NL_HPOINT,parameters,&basis,2,&curve,&stack);
    if(status!=NL_NO) return 2;
    // Independent quadratic endpoint coefficients, rather than the native
    // endpoint evaluator (which has the same clamped-row assumption).
    const auto *p=curve.pol->Pw;
    const double start=(4*p[0].x+5*p[1].x)/(4*p[0].w+5*p[1].w);
    const double end=(5*p[2].x+6*p[3].x)/(5*p[2].w+6*p[3].w);
    std::cout<<std::setprecision(17)<<"{\"expected_endpoints\":[0,2],\"fitted_endpoint_limits\":["
        <<start<<','<<end<<"],\"fitted_controls\":[";
    for(int i=0;i<4;++i) std::cout<<(i?",":"")<<p[i].x/p[i].w;
    std::cout<<"],\"expected_controls\":[-0.5,0.4,1.4,2.5]}"<<std::endl;
}
