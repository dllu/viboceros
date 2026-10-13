#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <iostream>

int main() {
    NL_STACKS stack;
    SmStackHandler cleanup(&stack);
    NL_RMATRIX band,full;
    if(N_SetRealMatrix(&band,1,1,NL_MT_BANDED,3,&stack)!=NL_NO ||
       N_SetRealMatrix(&full,1,1,NL_MT_FULL,1,&stack)!=NL_NO) return 2;
    double **b,**f;
    N_GetRealMatrixPtr(&band,&b);N_GetRealMatrixPtr(&full,&f);
    b[0][0]=0;b[0][1]=0;b[0][2]=1;
    b[1][0]=1;b[1][1]=0;b[1][2]=0;
    f[0][0]=0;f[0][1]=1;f[1][0]=1;f[1][1]=0;
    const auto band_status=N_RealMatrixLuDecompose(&band);
    NL_INDEX pivots[2];
    const auto full_status=N_RealMatrixLuDecomposePivot(&full,pivots);
    if(full_status!=NL_NO) return 3;
    double targets[]={1e-280,2e-280},answer[2];
    for(int i=0;i<2;++i) std::swap(targets[i],targets[pivots[i]]);
    const auto solve_status=N_RealMatrixRightForBack(&full,targets,answer);
    if(solve_status!=NL_NO) return 4;
    std::cout<<"{\"matrix\":[[0,1],[1,0]],\"band_status\":"<<band_status
        <<",\"pivoted_status\":"<<full_status<<",\"solve_status\":"<<solve_status
        <<",\"solution\":["<<answer[0]<<','<<answer[1]
        <<"],\"independent_solution\":[2e-280,1e-280]}\n";
    return band_status==NL_NO?5:0;
}
