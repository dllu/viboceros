#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstring>
#include <iomanip>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

namespace {
using H=std::array<double,4>;
using Rows=std::vector<std::vector<double>>;
struct Fixture {int kind,p;std::vector<double> knots;double parameter;bool safe;};
struct Raw {
    std::array<int,2> degree={-1,-1},count={0,0};
    std::array<std::vector<double>,2> knots;
    std::vector<H> h;
    bool operator==(const Raw&b)const {return degree==b.degree && count==b.count && knots==b.knots && h==b.h;}
};
void check(bool good){if(!good)throw std::runtime_error("exact split probe setup failed");}
H values(const NL_CPOINT&p){return {p.x,p.y,p.z,p.w};}
NL_CPOINT point(const H&h){return {h[0],h[1],h[2],h[3]};}
Raw read(const NL_CURVE&c){
    Raw r;if(!c.pol || !c.knt || c.pol->n<0 || c.pol->n>128 || c.knt->m<0 || c.knt->m>192)return r;
    r.degree={int(c.p),-1};r.count={int(c.pol->n+1),1};r.knots[0].assign(c.knt->U,c.knt->U+c.knt->m+1);
    for(NL_INDEX i=0;i<=c.pol->n;++i)r.h.push_back(values(c.pol->Pw[i]));
    return r;
}
Raw read(const NL_SURFACE&s){
    Raw r;if(!s.net || !s.knu || !s.knv || s.net->n<0 || s.net->m<0 || s.net->n>128 || s.net->m>128 || s.knu->m<0 || s.knv->m<0 || s.knu->m>192 || s.knv->m>192)return r;
    r.degree={int(s.p),int(s.q)};r.count={int(s.net->n+1),int(s.net->m+1)};
    r.knots[0].assign(s.knu->U,s.knu->U+s.knu->m+1);r.knots[1].assign(s.knv->U,s.knv->U+s.knv->m+1);
    for(int j=0;j<r.count[1];++j)for(int i=0;i<r.count[0];++i)r.h.push_back(values(s.net->Pw[i][j]));
    return r;
}
void allocate(const Raw&r,NL_CURVE&c,NL_STACKS&stack,bool poison=false){
    N_CrvInitArrays(&c);check(N_AllocCrvArrays(&c,r.count[0]-1,r.degree[0],r.knots[0].size()-1,&stack)==NL_NO);
    std::copy(r.knots[0].begin(),r.knots[0].end(),c.knt->U);
    for(size_t i=0;i<r.h.size();++i)c.pol->Pw[i]=poison?NL_CPOINT{NAN,NAN,NAN,NAN}:point(r.h[i]);
}
void allocate(const Raw&r,NL_SURFACE&s,NL_STACKS&stack,bool poison=false){
    N_SrfInitArrays(&s);check(N_AllocSrfArrays(&s,r.count[0]-1,r.count[1]-1,r.degree[0],r.degree[1],r.knots[0].size()-1,r.knots[1].size()-1,&stack)==NL_NO);
    std::copy(r.knots[0].begin(),r.knots[0].end(),s.knu->U);std::copy(r.knots[1].begin(),r.knots[1].end(),s.knv->U);
    for(int j=0;j<r.count[1];++j)for(int i=0;i<r.count[0];++i)s.net->Pw[i][j]=poison?NL_CPOINT{NAN,NAN,NAN,NAN}:point(r.h[j*r.count[0]+i]);
}
Raw oversized(Raw r,int kind){
    for(int d=0;d<(kind?2:1);++d){r.count[d]+=2;r.knots[d].resize(r.knots[d].size()+2,NAN);}
    r.h.resize(r.count[0]*r.count[1]);return r;
}
Rows blossom(int p,double a,double b){
    Rows result;
    for(int row=0;row<=p;++row){
        std::vector<double> c(1,1.);
        for(int factor=0;factor<p;++factor){const double t=factor<p-row?a:b;std::vector<double> next(c.size()+1,0.);
            for(size_t i=0;i<c.size();++i){next[i]+=(1-t)*c[i];next[i+1]+=t*c[i];}c=std::move(next);}
        result.push_back(std::move(c));
    }
    return result;
}
std::array<Rows,2> coefficients(const Fixture&f,double t,size_t count){
    std::array<Rows,2> out;
    const auto unit=[&](size_t index){check(index<count);std::vector<double> v(count,0.);v[index]=1.;return v;};
    const auto row=[&](std::initializer_list<double> weights,size_t offset=0){check(offset<=count && weights.size()<=count-offset);std::vector<double> v(count,0.);std::copy(weights.begin(),weights.end(),v.begin()+offset);return v;};
    const auto append=[&](int side,const Rows&rows,size_t offset){for(const auto &r:rows){check(offset<=count && r.size()<=count-offset);std::vector<double> v(count,0.);std::copy(r.begin(),r.end(),v.begin()+offset);out[side].push_back(v);}};
    if(f.kind==0){append(0,blossom(f.p,0,t),0);append(1,blossom(f.p,t,1),0);}
    else if(f.kind==1){
        if(t==0){auto c=row({.5,.5});out={Rows{unit(0),c},Rows{c,unit(1),unit(2),unit(3)}};}
        else if(t==1){auto c=row({.5,.5},1);out={Rows{unit(0),unit(1),c},Rows{c,unit(2),unit(3)}};}
        else if(t==2){auto c=row({.5,.5},2);out={Rows{unit(0),unit(1),unit(2),c},Rows{c,unit(3)}};}
        else if(t<1){auto c=row({.5*(1-t)*(1-t),.5+t-t*t,.5*t*t});out={Rows{unit(0),row({(1-t)/2,(1+t)/2}),c},Rows{c,row({1-t/2,t/2},1),unit(2),unit(3)}};}
        else{const double x=t-1;auto c=row({.5*(1-x)*(1-x),.5+x-x*x,.5*x*x},1);out={Rows{unit(0),unit(1),row({1-t/2,t/2},1),c},Rows{c,row({(3-t)/2,(t-1)/2},2),unit(3)}};}
    }else if(f.kind==2){
        const size_t offset=t==0?0:t==1?1:2;auto a=row({1./3,2./3},offset),b=row({2./3,1./3},offset+1),c=row({1./6,2./3,1./6},offset);
        for(size_t i=0;i<=offset;++i)out[0].push_back(unit(i));
        out[0].push_back(a);out[0].push_back(c);
        out[1]={c,b};for(size_t i=offset+2;i<count;++i)out[1].push_back(unit(i));
    }else if(f.kind==3){
        if(t==.5){for(size_t i=0;i<4;++i)out[0].push_back(unit(i));for(size_t i=4;i<8;++i)out[1].push_back(unit(i));}
        else if(t<.5){append(0,blossom(3,0,2*t),0);append(1,blossom(3,2*t,1),0);for(size_t i=4;i<8;++i)out[1].push_back(unit(i));}
        else{for(size_t i=0;i<4;++i)out[0].push_back(unit(i));append(0,blossom(3,0,2*t-1),4);append(1,blossom(3,2*t-1,1),4);}
    }else if(f.kind==4){const size_t cut=t==0?1:5;for(size_t i=0;i<cut;++i)out[0].push_back(unit(i));for(size_t i=cut;i<count;++i)out[1].push_back(unit(i));}
    else if(f.kind==5)out={Rows{unit(0),unit(1)},Rows{unit(1),unit(2)}};
    else out={Rows{unit(0)},Rows{unit(0)}};
    return out;
}
Raw tensor(const Raw&curve,int axis){
    Raw s;const int other=1-axis;s.degree[axis]=curve.degree[0];s.degree[other]=2;s.count[axis]=curve.count[0];s.count[other]=4;
    s.knots[axis]=curve.knots[0];s.knots[other]={-2,-1,0,.8,2,3,4};
    for(int j=0;j<s.count[1];++j)for(int i=0;i<s.count[0];++i){
        const auto &h=curve.h[axis?j:i];const int k=axis?i:j;const std::array<double,3> shift={3.*k,-2.*k,.5*k*k};H v;
        if(h[3]==NL_NOW){v=h;for(size_t c=0;c<3;++c)v[c]+=shift[c];}
        else{const double factor=1.+k/8.;for(size_t c=0;c<3;++c)v[c]=factor*(h[c]+h[3]*shift[c]);v[3]=factor*h[3];}
        s.h.push_back(v);
    }
    return s;
}
bool compare(const Raw&a,const Raw&b,double gauge,double&error,bool exact){
    if(a.degree!=b.degree || a.count!=b.count || a.knots!=b.knots || a.h.size()!=b.h.size())return false;
    bool good=true;
    for(size_t i=0;i<a.h.size();++i)for(size_t c=0;c<4;++c){
        if(c==3 && b.h[i][c]==NL_NOW){good &= a.h[i][c]==NL_NOW;continue;}
        const double difference=std::abs((a.h[i][c]-b.h[i][c])/gauge);good &= std::isfinite(a.h[i][c]) && difference<=5e-13*std::max(1.,std::abs(b.h[i][c]/gauge));
        if(std::isfinite(difference))error=std::max(error,difference);
        if(exact)good &= std::memcmp(&a.h[i][c],&b.h[i][c],sizeof(double))==0;
    }
    return good;
}
void array(const std::vector<double>&v){std::cout<<'[';for(size_t i=0;i<v.size();++i){if(i)std::cout<<',';if(std::isfinite(v[i]))std::cout<<v[i];else std::cout<<"null";}std::cout<<']';}
void print(const Raw&r){std::cout<<"{\"degree\":["<<r.degree[0]<<','<<r.degree[1]<<"],\"count\":["<<r.count[0]<<','<<r.count[1]<<"],\"knots\":[";array(r.knots[0]);std::cout<<',';array(r.knots[1]);std::cout<<"],\"controls\":[";for(size_t i=0;i<r.h.size();++i){if(i)std::cout<<',';array({r.h[i][0],r.h[i][1],r.h[i][2],r.h[i][3]});}std::cout<<"]}";}
using Storage=std::vector<const NL_REAL*>;
Storage storage(const NL_CURVE&c){
    Storage r;
    if(!c.pol || !c.knt || c.pol->n<0 || c.pol->n>128 || c.knt->m<0 || c.knt->m>192)return r;
    for(NL_INDEX i=0;i<=c.pol->n;++i){const auto &p=c.pol->Pw[i];r.insert(r.end(),{&p.x,&p.y,&p.z,&p.w});}
    for(NL_INDEX i=0;i<=c.knt->m;++i)r.push_back(c.knt->U+i);
    return r;
}
Storage storage(const NL_SURFACE&s){
    Storage r;
    if(!s.net || !s.knu || !s.knv || s.net->n<0 || s.net->m<0 || s.net->n>128 || s.net->m>128 || s.knu->m<0 || s.knv->m<0 || s.knu->m>192 || s.knv->m>192)return r;
    for(NL_INDEX i=0;i<=s.net->n;++i)for(NL_INDEX j=0;j<=s.net->m;++j){const auto &p=s.net->Pw[i][j];r.insert(r.end(),{&p.x,&p.y,&p.z,&p.w});}
    for(NL_INDEX i=0;i<=s.knu->m;++i)r.push_back(s.knu->U+i);
    for(NL_INDEX i=0;i<=s.knv->m;++i)r.push_back(s.knv->U+i);
    return r;
}
bool disjoint(const Storage&a,const Storage&b){
    if(a.empty() || b.empty())return false;
    return std::none_of(a.begin(),a.end(),[&](const NL_REAL*p){return std::find(b.begin(),b.end(),p)!=b.end();});
}
template<class T> bool independent(const T&source,const T&left,const T&right){
    const auto a=storage(source),b=storage(left),c=storage(right);
    return disjoint(a,b) && disjoint(a,c) && disjoint(b,c);
}
size_t invalid_requests(){
    size_t failures=0;
    Raw curve;curve.degree={3,-1};curve.count={4,1};curve.knots[0]={0,0,0,0,1,1,1,1};
    curve.h={{0,0,0,1},{1,2,0,1.25},{2,0,1,.75},{3,1,0,1.5}};
    for(int kind=0;kind<3;++kind)for(int mode=0;mode<12;++mode){
        const int axis=kind?kind-1:0;const Raw source=kind?tensor(curve,axis):curve;
        Raw left=source,right=source;left.knots[axis]={0,0,0,0,.375,.375,.375,.375};right.knots[axis]={.375,.375,.375,.375,1,1,1,1};
        if(mode==4){--left.count[axis];left.knots[axis].pop_back();left.h.resize(left.count[0]*left.count[1]);}
        if(mode==5){--right.count[axis];right.knots[axis].pop_back();right.h.resize(right.count[0]*right.count[1]);}
        double u=.375;
        if(mode==6)u=NAN;
        if(mode==7)u=INFINITY;
        if(mode==8)u=-.1;
        if(mode==9)u=1.1;
        if(mode==10)u=0;
        if(mode==11)u=1;
        NL_STACKS input_stack,output_stack;SmStackHandler input_cleanup(&input_stack),output_cleanup(&output_stack);
        NL_STACKS *allocation=mode>=3 && mode<=5?nullptr:&output_stack;
        int status;bool unchanged;
        if(!kind){
            NL_CURVE input,l,r;allocate(source,input,input_stack);N_CrvInitArrays(&l);N_CrvInitArrays(&r);
            if(mode!=3){allocate(left,l,output_stack,true);allocate(right,r,output_stack,true);}
            NL_CURVE *lp=mode==0?&input:&l,*rp=mode==1?&input:mode==2?&l:&r;
            status=N_CrvSplit(&input,u,lp,rp,allocation);unchanged=read(input)==source;
        }else{
            NL_SURFACE input,l,r;allocate(source,input,input_stack);N_SrfInitArrays(&l);N_SrfInitArrays(&r);
            if(mode!=3){allocate(left,l,output_stack,true);allocate(right,r,output_stack,true);}
            NL_SURFACE *lp=mode==0?&input:&l,*rp=mode==1?&input:mode==2?&l:&r;
            status=N_SrfSplit(&input,u,axis?NL_VDIR:NL_UDIR,lp,rp,allocation);unchanged=read(input)==source;
        }
        const bool passed=status!=NL_NO && unchanged;failures+=!passed;
        std::cout<<"{\"invalid\":true,\"kind\":"<<kind<<",\"mode\":"<<mode<<",\"status\":"<<status<<",\"source_unchanged\":"<<unchanged<<",\"passed\":"<<passed<<"}\n";
    }
    return failures;
}
}
int main(int argc,char**argv){
 try{
    const bool safe_only=argc>1 && std::string(argv[1])=="--safe-control";std::cout<<std::setprecision(17)<<std::boolalpha;
    std::vector<Fixture> fixtures;
    for(int p:{1,2,3,5,17}){std::vector<double> k(p+1,0.);k.insert(k.end(),p+1,1.);fixtures.push_back({0,p,k,.375,true});}
    fixtures.push_back({0,3,{0,0,0,0,1,1,1,1},1e-9,true});fixtures.push_back({0,3,{0,0,0,0,1,1,1,1},1-1e-9,true});
    for(double t:{0.,.375,1.,1+1e-9,1.625,2.})fixtures.push_back({1,2,{-2,-1,0,1,2,3,4},t,t!=0 && t!=2});
    for(double t:{0.,1.,2.})fixtures.push_back({2,3,{-3,-2,-1,0,1,2,3,4,5},t,t==1});
    for(double t:{.5,.5-1e-9,.5+1e-9})fixtures.push_back({3,3,{0,0,0,0,.5,.5,.5,.5,1,1,1,1},t,false});
    for(double t:{0.,1.})fixtures.push_back({4,3,{-1,0,0,0,0,1,1,1,1,2},t,false});
    fixtures.push_back({5,1,{0,0,1,1,1},1,false});fixtures.push_back({6,0,{0,1},.375,true});check(fixtures.size()==23);
    size_t failures=0,total=0;
    for(size_t id=0;id<fixtures.size();++id)for(int chart=0;chart<4;++chart)for(int representation=0;representation<3;++representation)
    for(int g=0;g<(representation?4:1);++g)for(int path=0;path<3;++path)for(int kind=0;kind<3;++kind){
        const auto &f=fixtures[id];if(safe_only && (chart!=1 || !f.safe))continue;++total;
        const double scale=std::ldexp(1.,chart==0?-360:chart==2?360:0),origin=chart==3?1e12:0.;const double gauge=g==0?1:g==1?-1:g==2?std::ldexp(1.,-300):std::ldexp(1.,300);
        Raw curve;curve.degree={f.p,-1};for(double k:f.knots)curve.knots[0].push_back(origin+scale*k);
        double u=origin+scale*f.parameter;
        if(f.kind==5){curve.knots[0][2]=std::nextafter(origin+scale,origin);u=curve.knots[0][2];}
        // A decimal offset can collapse after translation to 1e12. Author a
        // distinct representable parameter on the intended side of the group.
        if(f.kind!=5)for(double k:f.knots)if(f.parameter!=k && origin+scale*k==u)
            u=std::nextafter(u,f.parameter<k?-INFINITY:INFINITY);
        const double t=(u-origin)/scale;const size_t count=curve.knots[0].size()-f.p-1;curve.count={int(count),1};
        for(size_t i=0;i<count;++i){const double w=representation==2?gauge*(1.+(3*i%4)/4.):gauge;curve.h.push_back({2.*i*(representation?w:1.),double(i*i)*(representation?w:1.),double(int(i%3)-1)*(representation?w:1.),representation?w:NL_NOW});}
        const auto rows=coefficients(f,t,count);std::array<Raw,2> expected;
        for(int side=0;side<2;++side){auto &r=expected[side];r.degree=curve.degree;
            if(side==0){for(double k:curve.knots[0])if(k<u)r.knots[0].push_back(k);r.knots[0].insert(r.knots[0].end(),f.p+1,u);}
            else{r.knots[0].assign(f.p+1,u);for(double k:curve.knots[0])if(k>u)r.knots[0].push_back(k);}
            for(const auto &row:rows[side]){H h={0,0,0,0};auto one=std::find(row.begin(),row.end(),1.);
                if(one!=row.end() && std::count(row.begin(),row.end(),0.)+1==static_cast<long>(row.size()))h=curve.h[one-row.begin()];
                else{for(size_t i=0;i<count;++i)for(size_t c=0;c<(representation?4:3);++c)h[c]+=row[i]*curve.h[i][c];if(!representation)h[3]=NL_NOW;}
                r.h.push_back(h);
            }
            r.count={int(r.h.size()),1};check(r.knots[0].size()==r.h.size()+f.p+1);
        }
        Raw source=curve;if(kind){source=tensor(curve,kind-1);for(auto &r:expected)r=tensor(r,kind-1);}
        const bool exact=std::count(curve.knots[0].begin(),curve.knots[0].end(),u)==f.p+1;
        NL_STACKS output_stack;SmStackHandler output_cleanup(&output_stack);NL_CURVE cl,cr;NL_SURFACE sl,sr;N_CrvInitArrays(&cl);N_CrvInitArrays(&cr);N_SrfInitArrays(&sl);N_SrfInitArrays(&sr);
        if(path){const auto l=path==2?oversized(expected[0],kind):expected[0],r=path==2?oversized(expected[1],kind):expected[1];if(kind){allocate(l,sl,output_stack,true);allocate(r,sr,output_stack,true);}else{allocate(l,cl,output_stack,true);allocate(r,cr,output_stack,true);}}
        int status;bool unchanged,owned=false;std::array<Raw,2> before,after;
        {
            NL_STACKS input_stack;SmStackHandler input_cleanup(&input_stack);
            if(kind){NL_SURFACE input;allocate(source,input,input_stack);status=N_SrfSplit(&input,u,kind==1?NL_UDIR:NL_VDIR,&sl,&sr,path?nullptr:&output_stack);unchanged=read(input)==source;if(status==NL_NO){before={read(sl),read(sr)};owned=independent(input,sl,sr);}}
            else{NL_CURVE input;allocate(source,input,input_stack);status=N_CrvSplit(&input,u,&cl,&cr,path?nullptr:&output_stack);unchanged=read(input)==source;if(status==NL_NO){before={read(cl),read(cr)};owned=independent(input,cl,cr);}}
        }
        if(status==NL_NO)after=kind?std::array<Raw,2>{read(sl),read(sr)}:std::array<Raw,2>{read(cl),read(cr)};
        double error=0;const bool retained=status==NL_NO && before==after;const bool correct=status==NL_NO && compare(after[0],expected[0],gauge,error,exact) && compare(after[1],expected[1],gauge,error,exact);
        const bool passed=correct && retained && unchanged && owned;failures+=!passed;
        std::cout<<"{\"case\":"<<id<<",\"chart\":"<<chart<<",\"representation\":"<<representation<<",\"gauge\":"<<gauge<<",\"path\":"<<path<<",\"kind\":"<<kind<<",\"parameter\":"<<u<<",\"status\":"<<status<<",\"passed\":"<<passed<<",\"correct\":"<<correct<<",\"source_unchanged\":"<<unchanged<<",\"disjoint_storage\":"<<owned<<",\"retained_after_source_destruction\":"<<retained<<",\"full_order_payload_exact\":"<<(exact&&correct)<<",\"max_gauge_normalized_error\":"<<error<<",\"source\":";print(source);std::cout<<",\"expected\":[";print(expected[0]);std::cout<<',';print(expected[1]);std::cout<<"],\"actual\":[";print(after[0]);std::cout<<',';print(after[1]);std::cout<<"]}\n";
    }
    check(total==(safe_only?1053:7452));
    if(!safe_only)failures+=invalid_requests();
    return failures?1:0;
 }catch(const std::exception&e){std::cerr<<e.what()<<'\n';return 2;}
}
