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
struct Fixture {int kind,degree;std::vector<double> knots,parameters;};
using H = std::array<double,4>;
long double choose(int n,int k) {long double c=1;for(int i=1;i<=k;++i)c*=static_cast<long double>(n-i+1)/i;return c;}
std::vector<double> bernstein(int p,double t) {
    std::vector<double> result(p+1);
    for(int i=0;i<=p;++i)result[i]=choose(p,i)*std::pow(static_cast<long double>(t),i)*std::pow(1-static_cast<long double>(t),p-i);
    return result;
}
// Closed polynomial identities, independent of the native basis evaluator.
std::vector<double> coefficients(const Fixture &f,const std::vector<double>&k,double u,double origin,double scale) {
    std::vector<double> result(k.size()-f.degree-1,0.);
    const double t=(u-origin)/scale;
    if(f.kind==0)return bernstein(f.degree,t);
    if(f.kind==1 || f.kind==2) {
        const size_t offset=t<1?0:1;const double x=t-offset;
        if(f.kind==1) {result[offset]=.5*(1-x)*(1-x);result[offset+1]=.5+x-x*x;result[offset+2]=.5*x*x;}
        else {result[offset]=std::pow(1-x,3)/6;result[offset+1]=(3*x*x*x-6*x*x+4)/6;
            result[offset+2]=(-3*x*x*x+3*x*x+3*x+1)/6;result[offset+3]=x*x*x/6;}
    } else if(f.kind==3) {
        const double a=(k[3]-origin)/scale;
        if(t<a) {result[0]=(a-t)*(a-t)/(a*(a+1));result[2]=t*t/(2*a);result[1]=1-result[0]-result[2];}
        else {result[1]=(2-t)*(2-t)/(2*(2-a));result[3]=(t-a)*(t-a)/((3-a)*(2-a));result[2]=1-result[1]-result[3];}
    } else if(f.kind==4) {
        const size_t count=result.size();
        if(u==k[count])result[count-1]=1;
        else for(size_t i=1;i<count;++i)if(u>=k[i] && u<k[i+1]) {
            const double x=(u-k[i])/(k[i+1]-k[i]);result[i-1]=1-x;result[i]=x;break;
        }
    } else if(f.kind==5) {
        for(size_t i=0;i<result.size();++i)if((u>=k[i] && u<k[i+1]) || (i+1==result.size() && u==k[i+1])) {result[i]=1;break;}
    } else {
        const bool right=t>=.5;const auto b=bernstein(3,right?2*t-1:2*t);
        std::copy(b.begin(),b.end(),result.begin()+(right?4:0));
    }
    return result;
}
H values(const NL_CPOINT&p) {return {p.x,p.y,p.z,p.w};}
void array(const std::vector<double>&v) {std::cout<<'[';for(size_t i=0;i<v.size();++i){if(i)std::cout<<',';if(std::isfinite(v[i]))std::cout<<v[i];else std::cout<<"null";}std::cout<<']';}
void controls(const std::vector<H>&v) {std::cout<<'[';for(size_t i=0;i<v.size();++i){if(i)std::cout<<',';array({v[i][0],v[i][1],v[i][2],v[i][3]});}std::cout<<']';}
void check(bool ok) {if(!ok)throw std::runtime_error("point interpolation probe setup failed");}
}
int main() {
 try {
    std::cout<<std::setprecision(17)<<std::boolalpha;
    std::vector<Fixture> fixtures;
    for(int p:{1,2,3,5,11,17}) {
        std::vector<double> k(p+1,0.),parameters; k.insert(k.end(),p+1,1.);
        for(int i=0;i<=p;++i)parameters.push_back((1-std::cos(std::acos(-1.)*i/p))*.5);
        fixtures.push_back({0,p,k,parameters});
    }
    for(int p:{1,2,3,5}) {
        std::vector<double> k(p+1,0.),parameters;k.insert(k.end(),p+1,1.);
        for(int i=0;i<=p;++i)parameters.push_back(.125+.75*i/p);
        fixtures.push_back({0,p,k,parameters});
    }
    fixtures.push_back({1,2,{-2,-1,0,1,2,3,4},{0,.4,1.4,2}});
    fixtures.push_back({3,2,{-2,-1,0,.8,2,3,4},{0,.4,1.4,2}});
    fixtures.push_back({2,3,{-3,-2,-1,0,1,2,3,4,5},{0,.375,1,1.625,2}});
    fixtures.push_back({4,1,{0,0,.3,.7,1,1},{.05,.4,.6,.95}});
    fixtures.push_back({4,1,{-1,0,.3,.7,1,2},{0,.3,.7,1}});
    fixtures.push_back({5,0,{0,.2,.6,1},{.1,.4,.9}});
    fixtures.push_back({6,3,{0,0,0,0,.5,.5,.5,.5,1,1,1,1},{0,.125,.25,.375,.5,.625,.75,1}});
    size_t total=0,failed=0;
    for(size_t id=0;id<fixtures.size();++id)for(int chart=0;chart<4;++chart)
    for(int input_type=0;input_type<3;++input_type)for(int gauge_index=0;gauge_index<(input_type==1?3:1);++gauge_index)
    for(int order=0;order<3;++order)for(int path=0;path<2;++path) {
        ++total;const bool rational=input_type==1,homogeneous_input=input_type!=0;const auto &f=fixtures[id];const int exponent=chart==0?-360:chart==2?360:0;
        const double scale=std::ldexp(1.,exponent),origin=chart==3?1e12:0.;
        const double gauge=gauge_index==0?1.:gauge_index==1?std::ldexp(1.,-180):-std::ldexp(1.,180);
        std::vector<double> knots,stations;for(double k:f.knots)knots.push_back(origin+scale*k);
        for(double t:f.parameters)stations.push_back(origin+scale*t);
        const size_t count=stations.size();check(count==knots.size()-f.degree-1);
        std::vector<H> expected;
        for(size_t i=0;i<count;++i)expected.push_back({gauge*(.2*i+.1*(i%3)),gauge*(.125*i*i-.3*i),gauge*1e-18*(double(i)-double(count)/2),rational?gauge*(1.+.05*i):1.});
        std::vector<NL_CPOINT> homogeneous;std::vector<NL_POINT> euclidean;
        for(double u:stations) {
            const auto b=coefficients(f,knots,u,origin,scale);H sum={0,0,0,0};
            for(size_t c=0;c<4;++c) {long double value=0;for(size_t i=0;i<count;++i)value+=static_cast<long double>(b[i])*expected[i][c];sum[c]=value;}
            homogeneous.push_back({sum[0],sum[1],sum[2],input_type==2?NL_NOW:sum[3]});euclidean.push_back({sum[0],sum[1],sum[2]});
        }
        if(order==1) {std::reverse(stations.begin(),stations.end());std::reverse(homogeneous.begin(),homogeneous.end());std::reverse(euclidean.begin(),euclidean.end());}
        else if(order==2) {std::rotate(stations.begin(),stations.begin()+1,stations.end());std::rotate(homogeneous.begin(),homogeneous.begin()+1,homogeneous.end());std::rotate(euclidean.begin(),euclidean.begin()+1,euclidean.end());}
        const auto original_knots=knots,original_stations=stations;const auto original_h=homogeneous;const auto original_e=euclidean;
        NL_STACKS stack;SmStackHandler cleanup(&stack);NL_CURVE curve;N_CrvInitArrays(&curve);
        if(path)check(N_AllocCrvArrays(&curve,count-1,f.degree,knots.size()-1,&stack)==NL_NO);
        NL_KNOTVECTOR basis={static_cast<NL_INDEX>(knots.size()-1),knots.data()};
        const auto status=N_FitCrvInterpGivenParams(homogeneous_input?static_cast<void*>(homogeneous.data()):static_cast<void*>(euclidean.data()),count-1,homogeneous_input?NL_HPOINT:NL_EPOINT,stations.data(),&basis,f.degree,&curve,path?nullptr:&stack);
        bool unchanged=knots==original_knots && stations==original_stations;
        for(size_t i=0;i<count;++i)unchanged &= values(homogeneous[i])==values(original_h[i]) && euclidean[i].x==original_e[i].x && euclidean[i].y==original_e[i].y && euclidean[i].z==original_e[i].z;
        bool structure=status==NL_NO && curve.p==f.degree && curve.pol->n==static_cast<NL_INDEX>(count-1) && curve.knt->m==static_cast<NL_INDEX>(knots.size()-1);
        if(structure)structure=std::equal(knots.begin(),knots.end(),curve.knt->U);
        std::vector<H> actual,raw_actual;double control_error=0,point_error=0,homogeneous_error=0;
        bool finite=structure,points_correct=structure;
        if(structure) {
            // Input arrays are deliberately destroyed before querying the output.
            std::fill(knots.begin(),knots.end(),NAN);std::fill(stations.begin(),stations.end(),NAN);
            for(auto &h:homogeneous)h={NAN,NAN,NAN,NAN};
            for(auto &e:euclidean)e={NAN,NAN,NAN};
            H magnitudes={0,0,0,0};for(const auto &h:expected)for(size_t c=0;c<4;++c)magnitudes[c]=std::max(magnitudes[c],std::abs(h[c]/gauge));
            for(size_t i=0;i<count;++i) {
                auto h=values(curve.pol->Pw[i]);raw_actual.push_back(h);H normalized;
                if(!rational) {finite &= h[3]==NL_NOW;if(h[3]==NL_NOW)h[3]=1.;}
                for(size_t c=0;c<4;++c) {normalized[c]=h[c]/gauge;finite &= std::isfinite(normalized[c]);control_error=std::max(control_error,std::abs(normalized[c]-expected[i][c]/gauge)/magnitudes[c]);}
                actual.push_back(normalized);
            }
            for(int sample=0;sample<=32;++sample) {
                const double u=original_knots[f.degree]+(original_knots[count]-original_knots[f.degree])*sample/32;
                const auto b=coefficients(f,original_knots,u,origin,scale);H sum={0,0,0,0};
                for(size_t c=0;c<4;++c)for(size_t i=0;i<count;++i)sum[c]+=b[i]*expected[i][c]/gauge;
                NL_CPOINT h={};NL_POINT p={};points_correct &= N_CrvEvalPt(&curve,u,NL_LEFT,&h)==NL_NO && N_CrvEval(&curve,u,NL_LEFT,&p)==NL_NO;
                auto evaluated=values(h);const H projected={p.x,p.y,p.z,1.};
                if(!rational) {finite &= h.w==NL_NOW;if(h.w==NL_NOW)evaluated[3]=1.;}
                for(size_t c=0;c<4;++c) {finite &= std::isfinite(evaluated[c]) && std::isfinite(projected[c]);homogeneous_error=std::max(homogeneous_error,std::abs(evaluated[c]/gauge-sum[c])/magnitudes[c]);
                    if(c<3)point_error=std::max(point_error,std::abs(projected[c]-sum[c]/sum[3]));}
            }
        }
        const bool passed=structure && unchanged && finite && points_correct && control_error<=2e-8 && homogeneous_error<=2e-11 && point_error<=1e-10;
        std::cout<<"{\"case\":"<<id<<",\"chart\":"<<chart<<",\"degree\":"<<f.degree<<",\"homogeneous_input\":"<<homogeneous_input<<",\"rational\":"<<rational<<",\"gauge\":"<<gauge<<",\"order\":"<<order<<",\"preallocated\":"<<bool(path)<<",\"status\":"<<status<<",\"passed\":"<<passed<<",\"structure\":"<<structure<<",\"source_unchanged\":"<<unchanged<<",\"finite\":"<<finite<<",\"max_channel_relative_control_error\":"<<control_error<<",\"max_channel_relative_homogeneous_error\":"<<homogeneous_error<<",\"max_absolute_point_error\":"<<point_error<<",\"knots\":";
        array(original_knots);std::cout<<",\"parameters\":";array(original_stations);std::cout<<",\"expected_homogeneous_controls\":";
        auto normalized_expected=expected;for(auto &h:normalized_expected)for(auto &c:h)c/=gauge;controls(normalized_expected);
        std::cout<<",\"actual_homogeneous_controls\":";controls(actual);std::cout<<",\"actual_raw_controls\":";controls(raw_actual);std::cout<<"}\n";failed+=!passed;
    }
    check(total==2040);
#ifndef VIBOCEROS_POINT_INTERPOLATION_BASELINE
    // Malformed dimensions and nonfinite data are exercised only after their
    // guards exist; the original producer is not memory-safe on every input.
    for(int invalid=0;invalid<13;++invalid) {
        int p=1;std::vector<double> knots={0,0,.5,1,1},parameters={0,.5,1};
        std::vector<NL_CPOINT> data={{0,0,0,1},{1,1,0,1},{2,0,0,1}};
        switch(invalid) {
        case 0:parameters={0,0,1};break;
        case 1:parameters={1,0,0};break;
        case 2:parameters={-1,.5,1};break;
        case 3:parameters[1]=NAN;break;
        case 4:knots={0,0,.75,.5,1};break;
        case 5:data[1].x=INFINITY;break;
        case 6:data[1].w=NAN;break;
        case 7:
            p=3;knots={-1,0,0,0,0,1,1,1,1,2};parameters={0,.2,.4,.6,.8,1};data.resize(6,{1,0,0,1});break;
        case 8:
            p=2;knots={-2,-1,0,1,2,3,4};parameters={0,.1,.2,.3};data.resize(4,{1,0,0,1});break;
        case 9:p=0;knots={0,.2,.6,1};parameters={.1,.15,.9};break;
        case 10:p=3;knots={0,0,0,0,1,1,1};break;
        case 11:knots[2]=NAN;break;
        default:
            knots={0,0,1,1};parameters={.25,.75};data={{-1e308,0,0,1},{1e308,0,0,1}};break;
        }
        NL_STACKS stack;SmStackHandler cleanup(&stack);NL_CURVE curve;N_CrvInitArrays(&curve);
        NL_KNOTVECTOR basis={static_cast<NL_INDEX>(knots.size()-1),knots.data()};
        const auto status=N_FitCrvInterpGivenParams(data.data(),data.size()-1,NL_HPOINT,parameters.data(),&basis,p,&curve,&stack);
        const bool passed=status!=NL_NO;failed+=!passed;
        std::cout<<"{\"invalid\":true,\"case\":"<<invalid<<",\"status\":"<<status<<",\"passed\":"<<passed<<"}\n";
    }
#endif
    return failed?1:0;
 }catch(const std::exception &e) {std::cerr<<e.what()<<'\n';return 2;}
}
