// Qualify native removal status and distant polynomial extension independently.
#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <cmath>
#include <iomanip>
#include <iostream>

int main() {
    bool passed=true;
    std::cout<<std::setprecision(17)<<"{\"cases\":[";
    bool first=true;
    for(int power : {0,8,16,17,18,24,32,39}) {
        for(bool at_start : {false,true}) {
            NL_STACKS stack;
            SmStackHandler cleanup(&stack);
            // Homogeneous weight gauge .5 matches the adapter's normalization.
            NL_CPOINT points[]={{0,0,0,.5},{1,1.5,0,.5},{2,0,0,.5}};
            double knots[]={0,0,0,1,1,1};
            NL_CURVE source,result;
            N_CrvInitArrays(&source);N_CrvInitArrays(&result);
            if(N_CrvFromCPtsAndKnots(&source,points,2,2,knots,5,&stack)!=NL_NO) return 2;
            const auto target=at_start?-std::ldexp(1.,power):1.+std::ldexp(1.,power);
            N_ErrClear();
            const auto status=N_CrvExtendByParamDist(&source,target,at_start?NL_START:NL_END,
                                                     NL_CMAX,&result,&stack,&stack);
            if(!first)std::cout<<',';
            first=false;
            std::cout<<"{\"power\":"<<power<<",\"side\":\""<<(at_start?"start":"end")
                     <<"\",\"status\":"<<status<<",\"kernel_error_code\":"<<N_ErrGetType();
            if(status==NL_NO) {
                double source_error=0,relative_error=0;
                bool evaluated=true;
                const auto begin=result.knt->U[result.p];
                const auto end=result.knt->U[result.pol->n+1];
                for(int region=0;region<2;++region) for(int i=0;i<=64;++i) {
                    const auto t=region==0?i/64.:begin+(end-begin)*i/64.;
                    NL_POINT p;
                    if(N_CrvEval(&result,t,NL_LEFT,&p)!=NL_NO) {evaluated=false;break;}
                    const auto x=4*t,y=6*t*(1-t);
                    const auto residual=std::hypot(std::hypot(p.x-x,p.y-y),p.z);
                    if(region==0)source_error=std::max(source_error,residual);
                    relative_error=std::max(relative_error,residual/(1+std::hypot(x,y)));
                }
                std::cout<<",\"source_point_error\":"<<source_error
                         <<",\"relative_point_error\":"<<relative_error
                         <<",\"control_count\":"<<result.pol->n+1
                         <<",\"evaluated\":"<<(evaluated?"true":"false");
                passed=passed&&evaluated&&source_error<1e-9&&relative_error<1e-12;
            } else passed=false;
            std::cout<<'}';
        }
    }
    std::cout<<"],\"passed\":"<<(passed?"true":"false")<<"}\n";
    return passed?0:1;
}
