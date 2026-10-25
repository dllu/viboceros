#include <SmGeomUtility.h>
#include <array>
#include <iostream>

namespace {
bool geometric_c2(SmContinuityType value) {
    return value==SM_CT_G1_G2 || value==SM_CT_G1_G2_G3 || value>=SM_CT_C1_G2;
}
}

int main() {
    size_t count=0,failed=0;
    const std::array<std::array<double,2>,5> curvatures={{{0.,0.},{1.,0.},{2.,0.},{-1.,0.},{0.,1.}}};
    for(double shift:{0.,1000.})for(double left_speed:{.5,1.,4.})for(double right_speed:{.5,1.,4.})
        for(int acceleration=0;acceleration<3;++acceleration)for(int left=0;left<2;++left)
            for(const auto &right:curvatures) {
                const auto k=curvatures[left];
                SmPoint3d p(shift,-2.*shift,3.*shift),q=p;
                SmVector3d v(left_speed,0.,0.),w(right_speed,0.,0.);
                SmVector3d a(acceleration==0?0.:acceleration==1?-3.:4.,
                    left_speed*left_speed*k[0],left_speed*left_speed*k[1]);
                SmVector3d b(acceleration==0?0.:acceleration==1?5.:-2.,
                    right_speed*right_speed*right[0],right_speed*right_speed*right[1]);
                SmContinuityType continuity=SM_CT_UNDEFINED;
                const auto status=smgu_EvaluateCurveContinuity(p,v,a,q,w,b,continuity,1.);
                const bool expected=k==right;
                const bool passed=status==SM_SUCCESS && geometric_c2(continuity)==expected;
                if(!passed)++failed;
                std::cout<<"{\"case\":"<<count++<<",\"passed\":"<<(passed?"true":"false")
                    <<",\"geometric_c2\":"<<(geometric_c2(continuity)?"true":"false")
                    <<",\"expected\":"<<(expected?"true":"false")
                    <<",\"classification\":"<<int(continuity)<<"}\n";
            }
    std::cerr<<"Curve continuity: "<<count<<" cases, "<<failed<<" failures\n";
    return failed?1:0;
}
