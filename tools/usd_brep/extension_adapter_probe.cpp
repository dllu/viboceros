// Exercise the real C ABI and bounded native growth with sanitizer instrumentation.
#include <bridge.h>
#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <vector>

int main() {
    bool passed=true,first=true;
    std::cout<<std::setprecision(17)<<"{\"cases\":[";
    for(int power : {0,16,18,32,39}) for(bool at_start : {false,true})
    for(double gauge : {1.,1e-280,-1e280}) {
        double points[]={0,0,0,gauge,2,3,0,gauge,4,0,0,gauge};
        double knots[]={0,0,0,1,1,1};
        VbNurbsData input={{2,0},{3,1},{6,0},{knots,nullptr},points};
        const double begin=at_start?-std::ldexp(1.,power):0;
        const double end=at_start?1:1+std::ldexp(1.,power);
        VbSurfaceData *output=nullptr;
        char error[1024]={};
        const auto status=vb_nurbs_extend_curve(&input,begin,end,&output,error,sizeof(error));
        if(!first)std::cout<<',';
        first=false;
        std::cout<<"{\"power\":"<<power<<",\"side\":\""<<(at_start?"start":"end")
                 <<"\",\"gauge\":"<<gauge<<",\"status\":"<<status;
        if(status==0) {
            VbNurbsData result;
            if(vb_surface_data_view(output,&result)!=0) return 2;
            int exponent;std::frexp(std::abs(gauge),&exponent);
            std::vector<NL_CPOINT> hom;
            bool weights=true;
            for(size_t i=0;i<result.count[0];++i) {
                const auto *p=result.points_xyzw+4*i;
                const double w=std::ldexp(p[3],-exponent);
                weights=weights&&(p[3]==gauge);
                hom.push_back({p[0]*w,p[1]*w,p[2]*w,w});
            }
            NL_STACKS stack;SmStackHandler cleanup(&stack);
            NL_CURVE curve;N_CrvInitArrays(&curve);
            if(N_CrvFromCPtsAndKnots(&curve,hom.data(),hom.size()-1,2,
                    const_cast<double*>(result.knots[0]),result.knot_count[0]-1,&stack)!=NL_NO) return 3;
            double source_error=0,relative_error=0;
            bool evaluated=true;
            for(int region=0;region<2;++region) for(int i=0;i<=64;++i) {
                const auto t=region==0?i/64.:begin+(end-begin)*i/64.;
                NL_POINT p;
                if(N_CrvEval(&curve,t,NL_LEFT,&p)!=NL_NO) {evaluated=false;break;}
                const auto x=4*t,y=6*t*(1-t);
                const auto residual=std::hypot(std::hypot(p.x-x,p.y-y),p.z);
                if(region==0)source_error=std::max(source_error,residual);
                relative_error=std::max(relative_error,residual/(1+std::hypot(x,y)));
            }
            const bool domain=result.knots[0][2]==begin&&result.knots[0][result.count[0]]==end;
            std::cout<<",\"source_point_error\":"<<source_error<<",\"relative_point_error\":"<<relative_error
                     <<",\"weights_preserved\":"<<(weights?"true":"false")
                     <<",\"domain_preserved\":"<<(domain?"true":"false")
                     <<",\"evaluated\":"<<(evaluated?"true":"false");
            passed=passed&&weights&&domain&&evaluated&&source_error<1e-9&&relative_error<1e-12;
        } else {passed=false;std::cout<<",\"error\":\""<<error<<'"';}
        vb_surface_data_free(output);
        std::cout<<'}';
    }
    std::cout<<"],\"passed\":"<<(passed?"true":"false")<<"}\n";
    return passed?0:1;
}
