#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <iomanip>
#include <iostream>

int main() {
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_RMATRIX matrix;
    if(N_SetRealMatrix(&matrix,1,1,NL_MT_FULL,1,&stack)!=NL_NO) return 2;
    NL_REAL **a;
    N_GetRealMatrixPtr(&matrix,&a);
    a[0][0]=2;a[0][1]=1;a[1][0]=1;a[1][1]=2;
    NL_INDEX pivots[2];
    if(N_RealMatrixLuDecomposePivot(&matrix,pivots)!=NL_NO) return 3;
    const double tiny=1e-18;
    double real[]={tiny,0};
    if(N_RealMatrixForBackPivot(&matrix,pivots,real)!=NL_NO) return 4;
    NL_CPOINT targets[]={{tiny,0,0,0},{0,0,0,0}},result[2];
    for(int i=0;i<2;++i) std::swap(targets[i],targets[pivots[i]]);
    if(N_RealMatrixForBack(&matrix,targets,NL_HPOINT,result,&stack)!=NL_NO) return 5;
    // Independent inverse of [[2,1],[1,2]] gives [2*tiny/3,-tiny/3].
    std::cout<<std::setprecision(17)<<"{\"rhs\":["<<tiny<<",0],\"expected\":["<<2*tiny/3<<','<<-tiny/3
        <<"],\"real_pivot_solver\":["<<real[0]<<','<<real[1]
        <<"],\"permuted_homogeneous_solver\":["<<result[0].x<<','<<result[1].x<<"]}\n";
}
