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
struct Case { int kind,degree;std::vector<double> knots;double low,high; };
struct Curve {
    int degree=-1;
    std::vector<double> knots,h;
    bool operator==(const Curve &other)const{return degree==other.degree && knots==other.knots && h==other.h;}
};
void check(bool ok){if(!ok)throw std::runtime_error("curve segment limit probe setup failed");}
Curve read(const NL_CURVE &c){
    Curve r;
    if(!c.pol || !c.knt || c.pol->n<0 || c.pol->n>128 || c.knt->m<0 || c.knt->m>160)return r;
    r.degree=c.p;r.knots.assign(c.knt->U,c.knt->U+c.knt->m+1);
    for(NL_INDEX i=0;i<=c.pol->n;++i){const auto &p=c.pol->Pw[i];r.h.insert(r.h.end(),{p.x,p.y,p.z,p.w});}
    return r;
}
void allocate(const Curve &c,NL_CURVE &result,NL_STACKS &stack){
    N_CrvInitArrays(&result);
    check(N_AllocCrvArrays(&result,c.h.size()/4-1,c.degree,c.knots.size()-1,&stack)==NL_NO);
    std::copy(c.knots.begin(),c.knots.end(),result.knt->U);
    for(size_t i=0;i<c.h.size()/4;++i){const auto *p=c.h.data()+4*i;result.pol->Pw[i]={p[0],p[1],p[2],p[3]};}
}
// Coefficients of products (1-t+t*x), the independent Bernstein polar form.
std::vector<std::vector<double>> blossom(int degree,double a,double b){
    std::vector<std::vector<double>> result;
    for(int row=0;row<=degree;++row){
        std::vector<double> coefficients(1,1.);
        for(int factor=0;factor<degree;++factor){
            const double t=factor<degree-row?a:b;
            std::vector<double> next(coefficients.size()+1,0.);
            for(size_t k=0;k<coefficients.size();++k){next[k]+=(1-t)*coefficients[k];next[k+1]+=t*coefficients[k];}
            coefficients=std::move(next);
        }
        result.push_back(std::move(coefficients));
    }
    return result;
}
void array(const std::vector<double>&v){std::cout<<'[';for(size_t i=0;i<v.size();++i){if(i)std::cout<<',';if(std::isfinite(v[i]))std::cout<<v[i];else std::cout<<"null";}std::cout<<']';}
void print(const Curve &c){std::cout<<"{\"degree\":"<<c.degree<<",\"knots\":";array(c.knots);std::cout<<",\"homogeneous\":";array(c.h);std::cout<<'}';}
bool compare(const Curve &a,const Curve &b,double gauge,double &residual){
    if(a.degree!=b.degree || a.knots!=b.knots || a.h.size()!=b.h.size())return false;
    bool good=true;
    for(size_t i=0;i<a.h.size();++i){
        const double difference=std::abs((a.h[i]-b.h[i])/gauge),expected=std::abs(b.h[i]/gauge);
        good &= std::isfinite(a.h[i]) && std::isfinite(difference) && difference<=5e-13*std::max(1.,expected);
        if(std::isfinite(difference))residual=std::max(residual,difference);
    }
    return good;
}
}
int main(){
    try{
        std::cout<<std::setprecision(17)<<std::boolalpha;
        std::vector<Case> cases;
        for(int degree:{1,2,3,5,17}){
            std::vector<double> knots(degree+1,0.);knots.insert(knots.end(),degree+1,1.);
            cases.push_back({0,degree,knots,.125,.875});
        }
        cases.push_back({0,3,{0,0,0,0,1,1,1,1},1e-9,4e-9});
        cases.push_back({1,2,{-2,-1,0,1,2,3,4},0,2});
        cases.push_back({1,2,{-2,-1,0,1,2,3,4},.25,2});
        cases.push_back({1,2,{-2,-1,0,.8,2,3,4},.25,2});
        cases.push_back({2,3,{-3,-2,-1,0,1,2,3,4,5},0,2});
        cases.push_back({3,3,{-1,0,0,0,0,1,1,1,1,2},0,1});
        cases.push_back({3,3,{-1,0,0,0,0,1,1,1,1,2},.125,1});
        for(const auto &bounds:std::vector<std::array<double,2>>{{.125,.5},{.5,.875},{.25,.75},{0,1}})
            cases.push_back({4,3,{0,0,0,0,.5,.5,.5,.5,1,1,1,1},bounds[0],bounds[1]});
        check(cases.size()==16);
        size_t failures=0;
        for(size_t index=0;index<cases.size();++index)for(int chart=0;chart<4;++chart)
            for(bool rational:{false,true})for(double gauge:{1.,-1.,std::ldexp(1.,-300),std::ldexp(1.,300)}){
            const auto &test=cases[index];const int p=test.degree;
            const double scale=chart==0?std::ldexp(1.,-40):chart==2?std::ldexp(1.,40):1.;
            const double origin=chart==3?1e12:0.;
            Curve input,expected;input.degree=expected.degree=p;
            for(double k:test.knots)input.knots.push_back(origin+scale*k);
            double low=origin+scale*test.low,high=origin+scale*test.high;
            const size_t count=input.knots.size()-p-1;
            if(test.low>test.knots[p] && low==input.knots[p])low=std::nextafter(low,input.knots[count]);
            if(!(high>low))high=std::nextafter(low,input.knots[count]);
            const double a=(low-origin)/scale,b=(high-origin)/scale;
            for(size_t i=0;i<count;++i){
                double w=gauge*(rational?1.+double((3*i)%4)/4.:1.);
                std::array<double,3> point={2.*i,double(i*i),double(int(i%3)-1)};
                if(index==8){
                    point=std::array<std::array<double,3>,4>{{{-2,1,0},{0,4,1},{5,-2,-1},{8,3,2}}}[i];
                    if(rational)w=gauge*std::array<double,4>{.75,2.,.5,1.25}[i];
                }
                input.h.insert(input.h.end(),{point[0]*w,point[1]*w,point[2]*w,w});
            }
            std::vector<std::vector<double>> coefficients;
            auto append=[&](const std::vector<std::vector<double>> &block,size_t offset){
                for(const auto &row:block){std::vector<double> full(count,0.);std::copy(row.begin(),row.end(),full.begin()+offset);coefficients.push_back(std::move(full));}
            };
            if(test.kind==0 || test.kind==3)append(blossom(p,a,b),test.kind==3?1:0);
            else if(test.kind==1){
                const double sigma=(input.knots[3]-origin)/scale;
                const double first=(sigma-a)*(sigma-a)/((sigma+1)*sigma),last=a*a/(2*sigma);
                coefficients={{first,1-first-last,last,0},{0,1-a/2,a/2,0},{0,0,1,0},{0,0,1/(3-sigma),(2-sigma)/(3-sigma)}};
            }else if(test.kind==2){
                coefficients={{1./6.,4./6.,1./6.,0,0},{0,2./3.,1./3.,0,0},{0,0,1,0,0},{0,0,1./3.,2./3.,0},{0,0,1./6.,4./6.,1./6.}};
            }else{
                if(a<.5)append(blossom(p,2*a,2*std::min(b,.5)),0);
                if(b>.5)append(blossom(p,2*std::max(a,.5)-1,2*b-1),p+1);
            }
            expected.knots.assign(p+1,low);
            for(double k:input.knots)if(k>low && k<high)expected.knots.push_back(k);
            expected.knots.insert(expected.knots.end(),p+1,high);
            check(expected.knots.size()==coefficients.size()+p+1);
            expected.h.assign(4*coefficients.size(),0.);
            for(size_t i=0;i<coefficients.size();++i)for(size_t j=0;j<count;++j)for(size_t d=0;d<4;++d)
                expected.h[4*i+d]+=coefficients[i][j]*input.h[4*j+d];
            NL_STACKS output_stack;
            SmStackHandler output_cleanup(&output_stack);
            NL_CURVE allocated,preallocated;
            N_CrvInitArrays(&allocated);allocate(input,preallocated,output_stack);
            std::fill(preallocated.knt->U,preallocated.knt->U+input.knots.size(),42.);
            for(size_t i=0;i<count;++i)preallocated.pol->Pw[i]={111.,222.,333.,1.};
            std::array<NL_FLAG,3> status;
            std::array<Curve,3> actual;
            bool unchanged=false;
            {
                NL_STACKS source_stack;
                SmStackHandler source_cleanup(&source_stack);
                NL_CURVE source,inplace;
                allocate(input,source,source_stack);allocate(input,inplace,source_stack);
                status[0]=N_CrvExtractCrvSeg(&source,low,high,&allocated,&source_stack,&output_stack);
                status[1]=N_CrvExtractCrvSeg(&source,low,high,&preallocated,&source_stack,nullptr);
                status[2]=N_CrvExtractCrvSeg(&inplace,low,high,&inplace,&source_stack,&source_stack);
                unchanged=read(source)==input;
                if(status[0]==NL_NO)actual[0]=read(allocated);
                if(status[1]==NL_NO)actual[1]=read(preallocated);
                if(status[2]==NL_NO)actual[2]=read(inplace);
            }
            const bool independent=status[0]==NL_NO && status[1]==NL_NO
                && read(allocated)==actual[0] && read(preallocated)==actual[1];
            bool passed=unchanged && independent;
            std::cout<<"{\"case\":"<<index<<",\"chart\":"<<chart<<",\"rational\":"<<rational
                <<",\"gauge\":"<<gauge<<",\"low\":"<<low<<",\"high\":"<<high
                <<",\"source_unchanged\":"<<unchanged<<",\"independent_ownership\":"<<independent
                <<",\"input\":";print(input);std::cout<<",\"expected\":";print(expected);std::cout<<",\"outputs\":[";
            for(size_t mode=0;mode<3;++mode){
                double residual=0.;const bool correct=status[mode]==NL_NO && compare(actual[mode],expected,gauge,residual);passed &= correct;
                if(mode)std::cout<<',';
                std::cout<<"{\"mode\":"<<mode<<",\"status\":"<<status[mode]<<",\"correct\":"<<correct
                    <<",\"gauge_normalized_residual\":"<<residual<<",\"curve\":";print(actual[mode]);std::cout<<'}';
            }
            std::cout<<"]}\n";failures+=!passed;
        }
        return failures?1:0;
    }catch(const std::exception &error){std::cerr<<error.what()<<'\n';return 2;}
}
