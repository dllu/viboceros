#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <iomanip>
#include <iostream>

int main() {
    // The quadratic first-span continuation at -0.5 has independent basis
    // coefficients [9/8,-1/4,1/8], although the active domain starts at zero.
    double values[]={-1,-1,0,1,2,3,4,5,6,6};
    NL_KNOTVECTOR knots={9,values};
    double coefficients[3],*rows[]={coefficients};
    NL_INDEX span=-1;
    const auto status=N_BasisDerivs(&knots,2,-0.5,NL_LEFT,0,rows,&span);
    if(status!=NL_NO) return 2;
    std::cout<<std::setprecision(17)<<"{\"parameter\":-0.5,\"active_domain\":[0,5],\"status\":"<<status
        <<",\"span\":"<<span<<",\"native_coefficients\":["<<coefficients[0]<<','<<coefficients[1]<<','<<coefficients[2]
        <<"],\"independent_coefficients\":[1.125,-0.25,0.125]}"<<std::endl;
}
