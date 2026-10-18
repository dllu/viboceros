#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <stdexcept>
#include <vector>

namespace {
using Rows=std::vector<std::vector<double>>;
struct Case{int kind,p;std::vector<double> knots,queries;};
double choose(int n,int k){if(k<0 || k>n)return 0;double c=1;for(int i=1;i<=k;++i)c*=double(n-i+1)/i;return c;}
double bernstein(int p,int i,double t){return i<0 || i>p?0.:choose(p,i)*std::pow(t,i)*std::pow(1-t,p-i);}
Rows bernstein_rows(int p,int order,double t){
    Rows rows(order+1,std::vector<double>(p+1,0.));
    double factor=1;
    for(int d=0;d<=std::min(p,order);++d){
        if(d)factor*=p-d+1;
        for(int i=0;i<=p;++i)for(int j=0;j<=d;++j)
            rows[d][i]+=factor*((d-j)%2?-1:1)*choose(d,j)*bernstein(p-d,i-j,t);
    }
    return rows;
}
Rows expected(const Case &c,const std::vector<double>&knots,double u,double origin,double scale,int order,NL_FLAG side){
    const size_t count=knots.size()-c.p-1;
    Rows out(order+1,std::vector<double>(count,0.));
    const double t=(u-origin)/scale;
    auto put=[&](const Rows &rows,size_t offset,double derivative_scale){
        for(size_t d=0;d<rows.size();++d)for(size_t i=0;i<rows[d].size();++i)out[d][offset+i]=rows[d][i]*std::pow(derivative_scale,int(d));
    };
    if(c.kind==0 || c.kind==3)put(bernstein_rows(c.p,order,t),c.kind==3?1:0,1.);
    else if(c.kind==1 || c.kind==2){
        const size_t offset=t<1 || (t==1 && side==NL_RIGHT)?0:1;
        const double x=t-offset;
        if(c.kind==1){
            out[0][offset]=.5*(1-x)*(1-x);out[0][offset+1]=.5+x-x*x;out[0][offset+2]=.5*x*x;
            out[1][offset]=x-1;out[1][offset+1]=1-2*x;out[1][offset+2]=x;
            out[2][offset]=1;out[2][offset+1]=-2;out[2][offset+2]=1;
        }else{
            const double x2=x*x,x3=x2*x;
            const std::array<std::array<double,4>,4> rows={{{std::pow(1-x,3)/6,(3*x3-6*x2+4)/6,(-3*x3+3*x2+3*x+1)/6,x3/6},
                {-.5*(1-x)*(1-x),1.5*x2-2*x,-1.5*x2+x+.5,.5*x2},
                {1-x,3*x-2,1-3*x,x},{-1,3,-3,1}}};
            for(size_t d=0;d<rows.size() && d<out.size();++d)for(size_t i=0;i<4;++i)out[d][offset+i]=rows[d][i];
        }
    }else if(c.kind==4){
        const bool right=t>.5 || (t==.5 && side==NL_LEFT);
        put(bernstein_rows(c.p,order,right?2*t-1:2*t),right?c.p+1:0,2.);
    }else if(c.kind==5){
        const double sigma=(knots[3]-origin)/scale;
        if(t==0){
            out[0]={sigma/(sigma+1),1/(sigma+1),0,0};out[1]={-2/(sigma+1),2/(sigma+1),0,0};
            const double a=2/(sigma*(sigma+1)),b=1/sigma;out[2]={a,-a-b,b,0};
        }else{
            out[0]={0,0,1/(3-sigma),(2-sigma)/(3-sigma)};out[1]={0,0,-2/(3-sigma),2/(3-sigma)};
            // On the final span N1=(2-t)^2/[2(2-sigma)] and
            // N3=(t-sigma)^2/[(3-sigma)(2-sigma)].
            const double a=1/(2-sigma),b=2/((3-sigma)*(2-sigma));out[2]={0,a,-a-b,b};
        }
    }else if(c.kind==6){
        const double a=(knots[3]-origin)/scale,gap=1-a;
        const bool right=t>a || (t==a && side==NL_LEFT);
        if(t==0){out[0]={1,0,0,0};out[1]={-2/a,2/a,0,0};}
        else if(t==1){out[0]={0,0,0,1};out[1]={0,0,-2/gap,2/gap};}
        else{out[0]={0,gap,a,0};out[1]={0,-2,2,0};}
        out[2]=right?std::vector<double>{0,2/gap,(2*a-4)/(gap*gap),2/(gap*gap)}
                    :std::vector<double>{2/(a*a),(-2-2*a)/(a*a),2/a,0};
    }else{
        out[0][t<.5 || (t==.5 && side==NL_RIGHT)?0:1]=1.;
    }
    return out;
}
void array(const std::vector<double>&v){std::cout<<'[';for(size_t i=0;i<v.size();++i){if(i)std::cout<<',';if(std::isfinite(v[i]))std::cout<<v[i];else std::cout<<"null";}std::cout<<']';}
void point(const NL_POINT&p){array({p.x,p.y,p.z});}
bool close(double a,double b,double tolerance){return std::isfinite(a) && std::isfinite(b) && std::abs(a-b)<=tolerance*std::max(1.,std::abs(b));}
void check(bool ok){if(!ok)throw std::runtime_error("basis evaluation probe setup failed");}
std::array<double,4> values(const NL_CPOINT &p){return {p.x,p.y,p.z,p.w};}
}
int main(){
 try{
    std::cout<<std::setprecision(17)<<std::boolalpha;
    std::vector<Case> cases;
    for(int p:{1,2,3,5,17}){std::vector<double> k(p+1,0.);k.insert(k.end(),p+1,1.);cases.push_back({0,p,k,{0,.25,.5,1}});}
    cases.push_back({1,2,{-2,-1,0,1,2,3,4},{0,.5,1,1.5,2}});
    cases.push_back({5,2,{-2,-1,0,.8,2,3,4},{0,2}});
    cases.push_back({2,3,{-3,-2,-1,0,1,2,3,4,5},{0,.5,1,1.5,2}});
    cases.push_back({3,3,{-1,0,0,0,0,1,1,1,1,2},{0,.25,1}});
    cases.push_back({4,3,{0,0,0,0,.5,.5,.5,.5,1,1,1,1},{0,.25,.5,.75,1}});
    cases.push_back({6,2,{0,0,0,1,1,1,1},{0,0,1}});
    cases.push_back({7,0,{0,.5,1},{0,.5,1}});
    size_t failures=0,total=0;
    for(size_t index=0;index<cases.size();++index)for(int chart=0;chart<4;++chart)for(NL_FLAG side:{NL_LEFT,NL_RIGHT}){
        const auto &c=cases[index];const int p=c.p,exponent=chart==0?-360:chart==2?360:0;
        const double scale=std::ldexp(1.,exponent),origin=chart==3?1e12:0.;
        std::vector<double> knots;for(double k:c.knots)knots.push_back(origin+scale*k);
        std::vector<double> queries;for(double q:c.queries)queries.push_back(origin+scale*q);
        if(c.kind==6){knots[3]=std::nextafter(origin+scale,origin);queries[1]=knots[3];}
        const size_t count=knots.size()-p-1;const int order=chart==0 || chart==2?2:4;
        for(double u:queries){
            ++total;
            const auto want=expected(c,knots,u,origin,scale,order,side);
            NL_KNOTVECTOR knt={static_cast<NL_INDEX>(knots.size()-1),knots.data()};
            std::vector<double> basis(p+1),full(count,0.);
            NL_INDEX span=-1,dspan=-1;
            const auto value_status=N_BasisEval(&knt,p,u,side,basis.data(),&span);
            bool correct=value_status==NL_NO && span>=p && size_t(span)<count && knots[span]<knots[span+1];
            if(correct){for(int i=0;i<=p;++i)full[span-p+i]=basis[i];for(size_t i=0;i<count;++i)correct &= close(full[i],want[0][i],2e-13);}
            Rows derivatives(order+1,std::vector<double>(p+1));std::vector<double*> rows;
            for(auto &r:derivatives)rows.push_back(r.data());
            const auto derivative_status=N_BasisDerivs(&knt,p,u,side,order,rows.data(),&dspan);
            bool derivatives_correct=derivative_status==NL_NO && dspan>=p && size_t(dspan)<count && knots[dspan]<knots[dspan+1];
            Rows full_derivatives(order+1,std::vector<double>(count,0.));
            if(derivatives_correct){
                for(int d=0;d<=order;++d)for(int i=0;i<=p;++i)full_derivatives[d][dspan-p+i]=std::ldexp(derivatives[d][i],exponent*d);
                for(int d=0;d<=order;++d)for(size_t i=0;i<count;++i)derivatives_correct &= close(full_derivatives[d][i],want[d][i],2e-11);
            }
            std::cout<<"{\"case\":"<<index<<",\"chart\":"<<chart<<",\"right_limit\":"<<(side==NL_LEFT)
                <<",\"parameter\":"<<u<<",\"order\":"<<order<<",\"degree\":"<<p<<",\"knots\":";array(knots);
            std::cout<<",\"basis_status\":"<<value_status<<",\"basis_correct\":"<<correct<<",\"derivative_status\":"<<derivative_status
                <<",\"derivatives_correct\":"<<derivatives_correct<<",\"expected_basis\":";array(want[0]);std::cout<<",\"actual_basis\":";array(full);
            std::cout<<",\"curves\":[";
            bool passed=correct && derivatives_correct;
            size_t gauge_index=0;
            for(double gauge:{1.,std::ldexp(1.,-100),-std::ldexp(1.,100)}){
                if(gauge_index++)std::cout<<',';
                NL_STACKS stack;SmStackHandler cleanup(&stack);
                NL_CURVE curve;N_CrvInitArrays(&curve);
                check(N_AllocCrvArrays(&curve,count-1,p,knots.size()-1,&stack)==NL_NO);
                std::copy(knots.begin(),knots.end(),curve.knt->U);
                std::vector<NL_CPOINT> controls;
                for(size_t i=0;i<count;++i){const double w=gauge*(1.+double((3*i)%4)/4.);controls.push_back({2.*i*w,double(i*i)*w,double(int(i%3)-1)*w,w});curve.pol->Pw[i]=controls.back();}
                std::vector<std::array<double,4>> homogeneous(order+1,{0,0,0,0});
                for(int d=0;d<=order;++d)for(size_t i=0;i<count;++i){const auto h=values(controls[i]);for(size_t j=0;j<4;++j)homogeneous[d][j]+=want[d][i]*(h[j]/gauge);}
                std::vector<NL_POINT> euclidean(order+1,{0,0,0});
                for(int d=0;d<=order;++d)for(int j=0;j<3;++j){
                    double v=homogeneous[d][j];for(int r=1;r<=d;++r){const double previous=j==0?euclidean[d-r].x:j==1?euclidean[d-r].y:euclidean[d-r].z;v-=choose(d,r)*homogeneous[r][3]*previous;}
                    (j==0?euclidean[d].x:j==1?euclidean[d].y:euclidean[d].z)=v/homogeneous[0][3];
                }
                NL_CPOINT hpoint={};NL_POINT evaluated={};
                const auto hs=N_CrvEvalPt(&curve,u,side,&hpoint),ps=N_CrvEval(&curve,u,side,&evaluated);
                std::vector<NL_POINT> deriv(order+1,{NAN,NAN,NAN});
                const auto ds=N_CrvDerivs(&curve,u,side,order,deriv.data());
                bool points_correct=hs==NL_NO && ps==NL_NO;
                const auto hw=values(hpoint);
                for(size_t j=0;j<4;++j)points_correct &= close(hw[j]/gauge,homogeneous[0][j],5e-12);
                points_correct &= close(evaluated.x,euclidean[0].x,1e-10) && close(evaluated.y,euclidean[0].y,1e-10) && close(evaluated.z,euclidean[0].z,1e-10);
                bool curve_derivatives_correct=ds==NL_NO;
                if(curve_derivatives_correct)for(int d=0;d<=order;++d){
                    const double tolerance=d?2e-9:1e-10;
                    curve_derivatives_correct &= close(std::ldexp(deriv[d].x,exponent*d),euclidean[d].x,tolerance)
                        && close(std::ldexp(deriv[d].y,exponent*d),euclidean[d].y,tolerance) && close(std::ldexp(deriv[d].z,exponent*d),euclidean[d].z,tolerance);
                }
                NL_SURFACE surface;N_SrfInitArrays(&surface);check(N_AllocSrfArrays(&surface,count-1,1,p,1,knots.size()-1,3,&stack)==NL_NO);
                std::copy(knots.begin(),knots.end(),surface.knu->U);std::copy_n(std::array<double,4>{-3,-3,5,5}.data(),4,surface.knv->U);
                for(size_t i=0;i<count;++i){const auto &q=controls[i];surface.net->Pw[i][0]=q;surface.net->Pw[i][1]={2*q.x+6*q.w,2*q.y-4*q.w,2*q.z+10*q.w,2*q.w};}
                NL_POINT spoint={};const auto ss=N_SrfEvalPt(&surface,u,-1,side,NL_LEFT,&spoint);
                bool surface_correct=ss==NL_NO && close(spoint.x,euclidean[0].x+1.2,1e-10) && close(spoint.y,euclidean[0].y-.8,1e-10) && close(spoint.z,euclidean[0].z+2,1e-10);
                bool unchanged=std::equal(knots.begin(),knots.end(),curve.knt->U);
                for(size_t i=0;i<count;++i)unchanged &= values(curve.pol->Pw[i])==values(controls[i]);
                passed &= points_correct && curve_derivatives_correct && surface_correct && unchanged;
                std::cout<<"{\"gauge\":"<<gauge<<",\"points_correct\":"<<points_correct<<",\"derivatives_correct\":"<<curve_derivatives_correct
                    <<",\"surface_correct\":"<<surface_correct<<",\"source_unchanged\":"<<unchanged<<",\"homogeneous_status\":"<<hs
                    <<",\"point_status\":"<<ps<<",\"derivative_status\":"<<ds<<",\"surface_status\":"<<ss<<",\"expected_point\":";point(euclidean[0]);
                std::cout<<",\"actual_point\":";point(evaluated);std::cout<<",\"actual_surface_point\":";point(spoint);std::cout<<",\"expected_first_derivative_normalized\":";point(euclidean[1]);
                std::cout<<",\"actual_first_derivative_normalized\":";point({std::ldexp(deriv[1].x,exponent),std::ldexp(deriv[1].y,exponent),std::ldexp(deriv[1].z,exponent)});std::cout<<'}';
            }
            std::cout<<"]}\n";failures+=!passed;
        }
    }
    check(total==368);return failures?1:0;
 }catch(const std::exception &error){std::cerr<<error.what()<<'\n';return 2;}
}
