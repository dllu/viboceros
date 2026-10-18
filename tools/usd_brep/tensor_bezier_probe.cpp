#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstring>
#include <iomanip>
#include <iostream>
#include <set>
#include <stdexcept>
#include <string>
#include <vector>

namespace {
using H=std::array<double,4>;
using Rows=std::vector<std::vector<double>>;
struct Axis { int kind,p;std::vector<double> knots;bool safe; };
struct Raw {
    std::array<int,2> degree,count;
    std::array<std::vector<double>,2> knots;
    std::vector<H> h;
    bool operator==(const Raw&b)const{return degree==b.degree && count==b.count && knots==b.knots && h==b.h;}
};
void check(bool ok){if(!ok)throw std::runtime_error("tensor Bezier probe setup failed");}
Axis bezier(int p){std::vector<double> k(p+1,0.);k.insert(k.end(),p+1,1.);return {0,p,k,p>0};}
std::vector<Rows> coefficients(const Axis&a,const std::vector<double>&knots,double origin,double scale){
    const size_t count=knots.size()-a.p-1;
    const auto unit=[&](size_t i){check(i<count);std::vector<double> r(count,0.);r[i]=1.;return r;};
    const auto row=[&](std::initializer_list<double> v,size_t offset=0){check(offset<=count && v.size()<=count-offset);std::vector<double> r(count,0.);std::copy(v.begin(),v.end(),r.begin()+offset);return r;};
    std::vector<Rows> out;
    if(a.kind==0){Rows r;for(size_t i=0;i<count;++i)r.push_back(unit(i));out.push_back(r);}
    if(a.kind==1)out={{unit(0),unit(1),row({.5,.5},1)},{row({.5,.5},1),unit(2),unit(3)}};
    if(a.kind==2)out={{unit(0),unit(1),row({.5,.5},1),row({.25,.5,.25},1)},
                       {row({.25,.5,.25},1),row({.5,.5},2),unit(3),unit(4)}};
    if(a.kind==3){for(int block=0;block<2;++block){Rows r;for(int i=0;i<=a.p;++i)r.push_back(unit(block*(a.p+1)+i));out.push_back(r);}}
    if(a.kind==4)out={{row({.5,.5}),unit(1),row({.5,.5},1)},{row({.5,.5},1),unit(2),row({.5,.5},2)}};
    if(a.kind==5){for(size_t i=0;i<2;++i)out.push_back({row({1./6,2./3,1./6},i),row({2./3,1./3},i+1),row({1./3,2./3},i+1),row({1./6,2./3,1./6},i+1)});}
    if(a.kind==6)out={{unit(1),unit(2),unit(3),unit(4)}};
    if(a.kind==7)out={{unit(0),unit(1)},{unit(1),unit(2)}};
    if(a.kind==8)out={{unit(0)},{unit(1)}};
    if(a.kind==9)out={{row({.5,.5}),unit(1),row({.5,.5},1)}};
    if(a.kind==10){const double t=(knots[3]-origin)/scale;out={{row({t/(t+1),1/(t+1)}),unit(1),row({1-t/2,t/2},1)}};}
    check(!out.empty());return out;
}
Raw read(const NL_SURFACE&s){
    Raw r{{-1,-1},{0,0},{},{}};
    if(!s.net || !s.knu || !s.knv || s.net->n<0 || s.net->m<0 || s.net->n>128 || s.net->m>128 || s.knu->m<0 || s.knv->m<0 || s.knu->m>192 || s.knv->m>192)return r;
    r.degree={int(s.p),int(s.q)};r.count={int(s.net->n+1),int(s.net->m+1)};
    r.knots[0].assign(s.knu->U,s.knu->U+s.knu->m+1);r.knots[1].assign(s.knv->U,s.knv->U+s.knv->m+1);
    for(int j=0;j<r.count[1];++j)for(int i=0;i<r.count[0];++i){const auto &p=s.net->Pw[i][j];r.h.push_back({p.x,p.y,p.z,p.w});}
    return r;
}
void allocate(const Raw&r,NL_SURFACE&s,NL_STACKS&stack){
    N_SrfInitArrays(&s);check(N_AllocSrfArrays(&s,r.count[0]-1,r.count[1]-1,r.degree[0],r.degree[1],r.knots[0].size()-1,r.knots[1].size()-1,&stack)==NL_NO);
    std::copy(r.knots[0].begin(),r.knots[0].end(),s.knu->U);std::copy(r.knots[1].begin(),r.knots[1].end(),s.knv->U);
    for(int j=0;j<r.count[1];++j)for(int i=0;i<r.count[0];++i){const auto &h=r.h[j*r.count[0]+i];s.net->Pw[i][j]={h[0],h[1],h[2],h[3]};}
}
std::vector<const double*> storage(const NL_SURFACE&s){
    std::vector<const double*> r;
    for(NL_INDEX i=0;i<=s.net->n;++i)for(NL_INDEX j=0;j<=s.net->m;++j){const auto &p=s.net->Pw[i][j];r.insert(r.end(),{&p.x,&p.y,&p.z,&p.w});}
    for(NL_INDEX i=0;i<=s.knu->m;++i)r.push_back(s.knu->U+i);
    for(NL_INDEX i=0;i<=s.knv->m;++i)r.push_back(s.knv->U+i);
    return r;
}
size_t one_hot(const std::vector<double>&r){size_t index=r.size();for(size_t i=0;i<r.size();++i)if(r[i]!=0.){if(r[i]!=1. || index!=r.size())return r.size();index=i;}return index;}
bool compare(const Raw&a,const Raw&b,double gauge,double&error){
    if(a.degree!=b.degree || a.count!=b.count || a.knots!=b.knots || a.h.size()!=b.h.size())return false;
    bool ok=true;
    for(size_t i=0;i<a.h.size();++i)for(size_t c=0;c<4;++c){
        if(c==3 && b.h[i][c]==NL_NOW){ok &= a.h[i][c]==NL_NOW;continue;}
        const double delta=std::abs((a.h[i][c]-b.h[i][c])/gauge);
        ok &= std::isfinite(a.h[i][c]) && delta<=5e-13*std::max(1.,std::abs(b.h[i][c]/gauge));
        if(std::isfinite(delta))error=std::max(error,delta);
    }
    return ok;
}
void array(const std::vector<double>&v){std::cout<<'[';for(size_t i=0;i<v.size();++i){if(i)std::cout<<',';if(std::isfinite(v[i]))std::cout<<v[i];else std::cout<<"null";}std::cout<<']';}
void print(const Raw&r){std::cout<<"{\"degree\":["<<r.degree[0]<<','<<r.degree[1]<<"],\"count\":["<<r.count[0]<<','<<r.count[1]<<"],\"knots\":[";array(r.knots[0]);std::cout<<',';array(r.knots[1]);std::cout<<"],\"controls\":[";for(size_t i=0;i<r.h.size();++i){if(i)std::cout<<',';array({r.h[i][0],r.h[i][1],r.h[i][2],r.h[i][3]});}std::cout<<"]}";}
void print(const std::vector<Raw>&v){std::cout<<'[';for(size_t i=0;i<v.size();++i){if(i)std::cout<<',';print(v[i]);}std::cout<<']';}
}
int main(int argc,char**argv){
 try{
    const bool safe_only=argc>1 && std::string(argv[1])=="--safe-control";
    std::cout<<std::setprecision(17)<<std::boolalpha;
    const Axis regular2{1,2,{0,0,0,.5,1,1,1},true},regular3{2,3,{0,0,0,0,.5,1,1,1,1},true};
    const Axis full2{3,2,{0,0,0,.5,.5,.5,1,1,1},false},full3{3,3,{0,0,0,0,.5,.5,.5,.5,1,1,1,1},false};
    const Axis uniform2{4,2,{-2,-1,0,1,2,3,4},false},uniform3{5,3,{-3,-2,-1,0,1,2,3,4,5},false};
    const Axis inactive{6,3,{-1,0,0,0,0,1,1,1,1,2},false},thin{7,1,{0,0,1,1,1},true},constant{8,0,{0,.25,1},false};
    const Axis single{9,2,{-2,-1,0,1,2,3},false},irregular{10,2,{-2,-1,0,.8,2,3},false};
    std::vector<std::array<Axis,2>> pairs;
    for(const auto &p:std::vector<std::array<int,2>>{{0,0},{0,2},{2,0},{1,1},{2,2},{3,3},{5,3},{17,2},{2,17}})pairs.push_back({bezier(p[0]),bezier(p[1])});
    for(const auto &p:std::vector<std::array<Axis,2>>{{regular2,regular3},{regular3,regular2},{full2,full3},{full2,regular2},{regular2,full2},
        {uniform2,uniform2},{uniform3,uniform2},{uniform2,uniform3},{inactive,uniform2},{uniform2,inactive},{inactive,inactive},
        {thin,regular2},{regular2,thin},{constant,regular2},{regular2,constant},{single,single},{irregular,regular2},{regular2,irregular},{bezier(17),bezier(17)}})pairs.push_back(p);
    check(pairs.size()==28);size_t total=0,failures=0;
    for(size_t id=0;id<pairs.size();++id)for(int chart=0;chart<4;++chart)for(int rep=0;rep<3;++rep)for(int g=0;g<(rep?4:1);++g){
        const auto &axes=pairs[id];if(safe_only && (!axes[0].safe || !axes[1].safe))continue;++total;
        const double scale=std::ldexp(1.,chart==0?-360:chart==2?360:0),origin=chart==3?1e12:0.;
        const double gauge=g==0?1:g==1?-1:g==2?std::ldexp(1.,-300):std::ldexp(1.,300);
        Raw source;std::array<std::vector<Rows>,2> rows;std::array<std::vector<std::array<double,2>>,2> spans;
        for(int d=0;d<2;++d){const auto &a=axes[d];source.degree[d]=a.p;source.count[d]=a.knots.size()-a.p-1;
            for(double k:a.knots)source.knots[d].push_back(origin+scale*k);
            if(a.kind==7)source.knots[d][2]=std::nextafter(origin+scale,origin);
            rows[d]=coefficients(a,source.knots[d],origin,scale);
            for(int i=a.p;i<source.count[d];++i)if(source.knots[d][i]<source.knots[d][i+1])spans[d].push_back({source.knots[d][i],source.knots[d][i+1]});
            check(rows[d].size()==spans[d].size());
        }
        for(int j=0;j<source.count[1];++j)for(int i=0;i<source.count[0];++i){
            double w=rep==2?gauge*(1.+((3*i+5*j)%7)/8.):gauge;
            if(id==24 && rep==2){const double mixed[9]={1,-1,3,3,3,3,3,3,3};w=gauge*mixed[j*3+i];}
            const double factor=rep?w:1.;source.h.push_back({(.5*i+.125*j)*factor,(.75*j-.0625*i)*factor,(.25*((i%3)-(j%2))+i*j/32.)*factor,rep?w:NL_NOW});
        }
        std::vector<Raw> expected,actual;bool exact=true;
        for(size_t u=0;u<rows[0].size();++u)for(size_t v=0;v<rows[1].size();++v){
            Raw r;r.degree=source.degree;r.count={source.degree[0]+1,source.degree[1]+1};
            for(int d=0;d<2;++d){const auto &interval=spans[d][d?v:u];r.knots[d].assign(r.degree[d]+1,interval[0]);r.knots[d].insert(r.knots[d].end(),r.degree[d]+1,interval[1]);}
            for(const auto &b:rows[1][v])for(const auto &a:rows[0][u]){
                const size_t ia=one_hot(a),ib=one_hot(b);
                if(ia<a.size() && ib<b.size()){r.h.push_back(source.h[ib*source.count[0]+ia]);continue;}
                exact=false;H h{};for(size_t c=0;c<(rep?4:3);++c){long double value=0;for(size_t j=0;j<b.size();++j)for(size_t i=0;i<a.size();++i)value+=static_cast<long double>(a[i])*b[j]*source.h[j*source.count[0]+i][c];h[c]=static_cast<double>(value);}
                if(!rep)h[3]=NL_NOW;
                r.h.push_back(h);
            }
            expected.push_back(r);
        }
        NL_STACKS input_stack,output_stack;SmStackHandler output_cleanup(&output_stack);N_InitNurbs(&input_stack);
        NL_SURFACE input;allocate(source,input,input_stack);NL_SURFACE ***patches=nullptr;NL_INDEX ku=-1,kv=-1;
        const int status=N_SrfDecomposeToBez(&input,&patches,&ku,&kv,&output_stack);
        const bool unchanged=read(input)==source;
        bool layout=status==NL_NO && ku+1==static_cast<NL_INDEX>(rows[0].size()) && kv+1==static_cast<NL_INDEX>(rows[1].size()) && patches;
        bool independent=layout;std::set<const double*> addresses;for(auto *p:storage(input))addresses.insert(p);
        if(layout)for(NL_INDEX u=0;u<=ku;++u)for(NL_INDEX v=0;v<=kv;++v){actual.push_back(read(*patches[u][v]));for(auto *p:storage(*patches[u][v]))independent &= addresses.insert(p).second;}
        N_EndNurbs(&input_stack);
        bool survives=layout;
        if(layout){size_t i=0;for(NL_INDEX u=0;u<=ku;++u)for(NL_INDEX v=0;v<=kv;++v)survives &= read(*patches[u][v])==actual[i++];}
        bool matches=actual.size()==expected.size();double error=0;
        if(matches)for(size_t i=0;i<expected.size();++i){matches &= compare(actual[i],expected[i],gauge,error);if(exact && actual[i].h.size()==expected[i].h.size())matches &= std::memcmp(actual[i].h.data(),expected[i].h.data(),actual[i].h.size()*sizeof(H))==0;}
        const bool passed=layout && unchanged && independent && survives && matches;failures+=!passed;
        std::cout<<"{\"fixture\":"<<id<<",\"chart\":"<<chart<<",\"representation\":"<<rep<<",\"gauge\":"<<gauge<<",\"status\":"<<status<<",\"source_unchanged\":"<<unchanged<<",\"independent_storage\":"<<independent<<",\"survives_input_cleanup\":"<<survives<<",\"exact_copy\":"<<exact<<",\"max_scaled_control_error\":"<<error<<",\"source\":";print(source);std::cout<<",\"expected\":";print(expected);std::cout<<",\"actual\":";print(actual);std::cout<<",\"passed\":"<<passed<<"}\n";
    }
    std::cerr<<total<<" tensor Bezier cases, "<<failures<<" failures\n";return failures?1:0;
 }catch(const std::exception&e){std::cerr<<e.what()<<'\n';return 2;}
}
