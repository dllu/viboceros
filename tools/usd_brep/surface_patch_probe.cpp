#include <SmApiGeneral.h>
#include <SmBSplineSurface.h>
#include <SmNurbsCrv.h>
#include <nurbs.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <vector>

namespace {
struct Axis {
    int kind, degree;
    std::vector<double> knots;
    double low, high;
    bool next = false;
};
struct Prepared {
    int degree;
    std::vector<double> input, output;
    std::vector<std::vector<double>> coefficients;
    double low, high;
};
std::vector<std::vector<double>> blossom(int degree, double a, double b) {
    if (degree == 1) return {{1-a,a},{1-b,b}};
    return {{(1-a)*(1-a),2*a*(1-a),a*a},
            {(1-a)*(1-b),a+b-2*a*b,a*b},
            {(1-b)*(1-b),2*b*(1-b),b*b}};
}
Prepared prepare(const Axis &axis, double origin, double scale) {
    Prepared p;
    p.degree=axis.degree;
    for (double k:axis.knots) p.input.push_back(origin+scale*k);
    p.low=origin+scale*axis.low;p.high=origin+scale*axis.high;
    const size_t count=axis.knots.size()-axis.degree-1;
    if (axis.next || (axis.low>axis.knots[axis.degree] && p.low==p.input[axis.degree]))
        p.low=std::nextafter(p.low,origin+scale*axis.knots[count]);
    if (!(p.high>p.low)) p.high=std::nextafter(p.low,origin+scale*axis.knots[count]);
    const double a=(p.low-origin)/scale,b=(p.high-origin)/scale;
    p.output.assign(axis.degree+1,p.low);
    for (double k:p.input) if (k>p.low && k<p.high) p.output.push_back(k);
    p.output.insert(p.output.end(),axis.degree+1,p.high);
    auto append=[&](const std::vector<std::vector<double>> &rows,size_t offset) {
        for (const auto &row:rows) {
            std::vector<double> full(count,0.);
            std::copy(row.begin(),row.end(),full.begin()+offset);
            p.coefficients.push_back(std::move(full));
        }
    };
    if (axis.kind==0 || axis.kind==2) append(blossom(axis.degree,a,b),axis.kind==2?1:0);
    else if (axis.kind==1) {
        if (a==0 && b==2) p.coefficients={{.5,.5,0,0},{0,1,0,0},{0,0,1,0},{0,0,.5,.5}};
        else {
            const size_t offset=a>=1?1:0;
            const auto bez=blossom(2,a-offset,b-offset);
            // A uniform quadratic span's Bernstein controls are
            // (P_i+P_(i+1))/2, P_(i+1), (P_(i+1)+P_(i+2))/2.
            for (const auto &row:bez) {
                std::vector<double> full(count,0.);
                full[offset]=row[0]/2;
                full[offset+1]=row[0]/2+row[1]+row[2]/2;
                full[offset+2]=row[2]/2;
                p.coefficients.push_back(std::move(full));
            }
        }
    } else {
        if (a<.5) append(blossom(2,2*a,2*std::min(b,.5)),0);
        if (b>.5) append(blossom(2,2*std::max(a,.5)-1,2*b-1),3);
    }
    return p;
}
struct Snapshot {
    int p=-1,q=-1;
    size_t nu=0,nv=0;
    std::vector<double> u,v,h;
    bool operator==(const Snapshot &other) const {
        return p==other.p && q==other.q && nu==other.nu && nv==other.nv
            && u==other.u && v==other.v && h==other.h;
    }
};
Snapshot snapshot(const NL_SURFACE &surface) {
    Snapshot s;
    if (!surface.net || !surface.knu || !surface.knv || surface.net->n<0 || surface.net->m<0
        || surface.net->n>64 || surface.net->m>64 || surface.knu->m>80 || surface.knv->m>80) return s;
    s.p=surface.p;s.q=surface.q;s.nu=surface.net->n+1;s.nv=surface.net->m+1;
    s.u.assign(surface.knu->U,surface.knu->U+surface.knu->m+1);
    s.v.assign(surface.knv->U,surface.knv->U+surface.knv->m+1);
    for (size_t i=0;i<s.nu;++i) for (size_t j=0;j<s.nv;++j) {
        const auto &p=surface.net->Pw[i][j];
        s.h.insert(s.h.end(),{p.x,p.y,p.z,p.w});
    }
    return s;
}
Snapshot snapshot(SmBSplineSurface &surface) { return snapshot(*surface.GetOrCreateGwNurbPointer()); }
void array(const std::vector<double> &values) {
    std::cout<<'[';
    for (size_t i=0;i<values.size();++i) {
        if (i) std::cout<<',';
        if (std::isfinite(values[i])) std::cout<<values[i]; else std::cout<<"null";
    }
    std::cout<<']';
}
void print(const Snapshot &s) {
    std::cout<<"{\"degree_u\":"<<s.p<<",\"degree_v\":"<<s.q
        <<",\"count_u\":"<<s.nu<<",\"count_v\":"<<s.nv<<",\"knots_u\":";
    array(s.u);std::cout<<",\"knots_v\":";array(s.v);
    std::cout<<",\"homogeneous_u_major\":";array(s.h);std::cout<<'}';
}
bool compare(const Snapshot &got,const Snapshot &want,double &residual) {
    if (got.p!=want.p || got.q!=want.q || got.nu!=want.nu || got.nv!=want.nv
        || got.u!=want.u || got.v!=want.v || got.h.size()!=want.h.size()) return false;
    bool correct=true;
    for (size_t i=0;i<got.h.size();++i) {
        const double error=std::abs(got.h[i]-want.h[i]);
        correct &= std::isfinite(got.h[i]) && error<=2e-13*std::max(1.,std::abs(want.h[i]));
        if (std::isfinite(error)) residual=std::max(residual,error);
    }
    return correct;
}
void check(bool success) { if (!success) throw std::runtime_error("surface patch probe setup failed"); }
}

int main() {
    try {
        std::cout<<std::setprecision(17)<<std::boolalpha;
        const Axis line{0,1,{0,0,1,1},1e-9,.75};
        const Axis quad{0,2,{0,0,0,1,1,1},.25,.75};
        const Axis uniform{1,2,{-2,-1,0,1,2,3,4},0,2};
        const Axis inactive{2,2,{-1,0,0,0,1,1,1,2},0,1};
        const Axis broken{3,2,{0,0,0,.5,.5,.5,1,1,1},0,1};
        Axis line_v=line;line_v.low=.125;line_v.high=.875;
        Axis thin=quad;thin.low=1e-9;thin.high=4e-9;
        Axis after=uniform;after.low=1;after.high=1.75;after.next=true;
        Axis before=uniform;before.low=.125;before.high=.75;
        Axis left=broken;left.low=.25;left.high=.5;
        Axis right=broken;right.low=.5;right.high=.75;
        Axis right_end=broken;right_end.low=.5;
        Axis across=broken;across.low=.25;across.high=.75;
        const std::vector<std::array<Axis,2>> cases={
            {line,line_v},{thin,quad},{after,before},{uniform,uniform},
            {inactive,uniform},{left,quad},{right,right_end},{across,broken}};
        const auto &context=*SmApiGetOrCreateContext();
        size_t failures=0;
        for (size_t index=0;index<cases.size();++index) for (int chart=0;chart<4;++chart)
            for (bool swapped:{false,true}) for (bool rational:{false,true}) for (double gauge:{1.,-1.}) {
            const double scale=chart==0?std::ldexp(1.,-40):chart==2?std::ldexp(1.,40):1.;
            const auto u=prepare(cases[index][swapped?1:0],chart==3?1e12:0.,scale);
            const auto v=prepare(cases[index][swapped?0:1],chart==3?-2e12:0.,2*scale);
            Snapshot input,expected;
            input.p=u.degree;input.q=v.degree;input.u=u.input;input.v=v.input;
            input.nu=input.u.size()-input.p-1;input.nv=input.v.size()-input.q-1;
            std::vector<NL_CPOINT> controls;
            std::vector<NL_CPOINT*> rows(input.nu);
            for (size_t i=0;i<input.nu;++i) for (size_t j=0;j<input.nv;++j) {
                const double w=gauge*(rational?1.+double((3*i+5*j)%4)/4.:1.);
                controls.push_back({double(i)*w,double(j*j)*w,double(i*j+1)*w,w});
                input.h.insert(input.h.end(),{controls.back().x,controls.back().y,controls.back().z,w});
            }
            for (size_t i=0;i<input.nu;++i) rows[i]=controls.data()+i*input.nv;
            expected.p=u.degree;expected.q=v.degree;expected.u=u.output;expected.v=v.output;
            expected.nu=u.coefficients.size();expected.nv=v.coefficients.size();
            expected.h.assign(expected.nu*expected.nv*4,0.);
            for (size_t i=0;i<expected.nu;++i) for (size_t j=0;j<expected.nv;++j)
                for (size_t a=0;a<input.nu;++a) for (size_t b=0;b<input.nv;++b)
                    for (size_t d=0;d<4;++d) expected.h[4*(i*expected.nv+j)+d]
                        +=u.coefficients[i][a]*v.coefficients[j][b]*input.h[4*(a*input.nv+b)+d];
            check(expected.u.size()==expected.nu+expected.p+1 && expected.v.size()==expected.nv+expected.q+1);
            NL_STACKS stack;
            SmStackHandler cleanup(&stack);
            NL_SURFACE source,inplace,patch;
            N_SrfInitArrays(&source);N_SrfInitArrays(&inplace);N_SrfInitArrays(&patch);
            check(N_SrfFromCPtsAndKnots(&source,rows.data(),input.nu-1,input.nv-1,input.p,input.q,
                input.u.data(),input.v.data(),input.u.size()-1,input.v.size()-1,&stack)==NL_NO);
            check(N_SrfCopy(&source,&inplace,&stack)==NL_NO);
            const auto native_status=N_SrfExtractPatch(&source,u.low,u.high,v.low,v.high,&patch,&stack,&stack);
            const auto in_place_status=N_SrfExtractPatch(&inplace,u.low,u.high,v.low,v.high,&inplace,&stack,&stack);
            std::unique_ptr<SmBSplineSurface> object(new(context) SmBSplineSurface);
            check(object->SetExpert(input.p,input.q,SM_SF_UNSPECIFIED,SM_EK_CLAMPPED,
                input.u.size(),input.u.data(),input.v.size(),input.v.data(),SM_CP_HOMOGENEOUS_RATIONAL,
                input.nv*4,4,input.h.data())==SM_SUCCESS);
            SmBSplineSurface *raw=nullptr;
            const auto object_status=object->CopySubPatch(context,SmExtent2d(u.low,v.low,u.high,v.high),raw);
            std::unique_ptr<SmBSplineSurface> owned(raw);
            const bool unchanged=snapshot(source)==input && snapshot(*object)==input;
            const std::array<SmStatus,3> status={native_status,in_place_status,object_status};
            const std::array<Snapshot,3> actual={native_status==NL_NO?snapshot(patch):Snapshot{},
                in_place_status==NL_NO?snapshot(inplace):Snapshot{},owned?snapshot(*owned):Snapshot{}};
            object.reset();
            const bool independent=owned && snapshot(*owned)==actual[2];
            std::cout<<"{\"case\":"<<index<<",\"chart\":"<<chart<<",\"swapped\":"<<swapped
                <<",\"rational\":"<<rational<<",\"gauge\":"<<gauge
                <<",\"source_unchanged\":"<<unchanged<<",\"independent_ownership\":"<<independent
                <<",\"u\":["<<u.low<<','<<u.high<<"],\"v\":["<<v.low<<','<<v.high<<"],\"input\":";
            print(input);std::cout<<",\"expected\":";print(expected);std::cout<<",\"outputs\":[";
            bool passed=unchanged && independent;
            for (size_t mode=0;mode<3;++mode) {
                double residual=0.;
                const bool correct=status[mode]==(mode==2?SM_SUCCESS:NL_NO) && compare(actual[mode],expected,residual);
                passed &= correct;
                if (mode) std::cout<<',';
                std::cout<<"{\"mode\":"<<mode<<",\"status\":"<<status[mode]<<",\"correct\":"<<correct
                    <<",\"residual\":"<<residual<<",\"surface\":";print(actual[mode]);std::cout<<'}';
            }
            failures+=!passed;
            std::cout<<"]}\n";
        }
        return failures?1:0;
    } catch (const std::exception &error) {
        std::cerr<<error.what()<<'\n';return 2;
    }
}
