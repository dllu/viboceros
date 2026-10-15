#include "brep_data.h"
#include <SmBrep.h>
#include <SmFace.h>
#include <SmSurface.h>
#include <SmBSplineSurface.h>
#include <SmBSplineCurve.h>
#include <algorithm>
#include <cmath>
#include <memory>
#include <stdexcept>
#include <unordered_set>

namespace {
void require(bool ok,const char *message){if(!ok)throw std::runtime_error(message);}
// Continuity is computed by the kernel, rather than inferred from knot
// multiplicity. A repeated knot can describe a perfectly smooth surface.
bool requested(SmContinuityType c,bool kinks,bool tangents){
    return (kinks && c==SM_CT_C0) ||
           (tangents && (c==SM_CT_G1 || c==SM_CT_G1R || c==SM_CT_C1));
}
size_t controls(const SmSurface *surface){
    require(surface && surface->IsKindOf(SmBSplineSurface_TYPE),"Crease division requires an exact NURBS surface");
    const auto *s=static_cast<const SmBSplineSurface*>(surface);
    const size_t u=s->GetNumberControlPoints(SM_SP_U),v=s->GetNumberControlPoints(SM_SP_V);
    require(v>0 && u>0 && u<=2000000/v,"Crease surface control resource limit");
    return u*v;
}
}

size_t vb_divide_face_creases(SmBrep &brep,bool kinks,bool tangents,double tolerance){
    if(!kinks && !tangents)return 0;
    SmTemporaryChangeValue<SmBoolean> editing(brep.m_bEditingEnabled,TRUE);
    SmTArray<SmFace*> original;
    brep.GetFaces(original);
    require(original.GetSize()>0 && original.GetSize()<=512,"Crease division input face resource limit");
    std::vector<SmSurface*> surfaces;
    std::unordered_set<SmSurface*> seen;
    size_t input_controls=0;
    for(ULONG f=0;f<original.GetSize();++f){
        auto *surface=original[f]->GetSurface();
        require(surface,"Missing crease surface");
        input_controls+=controls(surface);
        require(input_controls<=2000000,"Crease input control resource limit");
        if(seen.insert(surface).second)surfaces.push_back(surface);
    }
    size_t line_count=0;
    for(auto *surface:surfaces){
        require(surface->IsKindOf(SmBSplineSurface_TYPE),"Crease division requires an exact NURBS surface");
        const auto domain=surface->GetNaturalUVDomain();
        std::vector<double> cuts[2];
        for(int d=0;d<2;++d){
            const auto direction=d?SM_SP_V:SM_SP_U;
            SmTArray<double> knots;
            require(surface->GetKnots(direction,knots)==SM_SUCCESS && knots.GetSize()<=2048,"Crease knot resource limit");
            SmContinuityType minimum;
            SmTArray<SmContinuityType> continuity;
            require(surface->CalculateContinuities(direction,minimum,continuity)==SM_SUCCESS && continuity.GetSize()==knots.GetSize(),"Classify native surface continuity");
            const double lo=d?domain.GetVMin():domain.GetUMin(),hi=d?domain.GetVMax():domain.GetUMax();
            for(ULONG i=1;i+1<knots.GetSize();++i){
                require(std::isfinite(knots[i]),"Nonfinite crease parameter");
                if(knots[i]>lo && knots[i]<hi && requested(continuity[i],kinks,tangents))cuts[d].push_back(knots[i]);
            }
            std::sort(cuts[d].begin(),cuts[d].end());
            cuts[d].erase(std::unique(cuts[d].begin(),cuts[d].end()),cuts[d].end());
            line_count+=cuts[d].size();
            require(line_count<=1024,"Crease split-line resource limit");
        }
        if(cuts[0].empty() && cuts[1].empty())continue;
        require((cuts[0].size()+1)*(cuts[1].size()+1)<=16384,"Crease patch resource limit");
        std::vector<std::unique_ptr<SmCurve>> owners;
        SmTArray<SmCurve*> curves,uv_curves;
        for(int d=0;d<2;++d)for(double t:cuts[d]){
            SmBSplineCurve *raw=nullptr;
            SmBSplineCurve *uv=nullptr;
            // The BSpline override extracts the exact tensor isocurve; its
            // approximation argument is unused. Keep the exact export gate.
            const auto status=surface->CreateIsoParametricCurve(*brep.GetContext(),d?SM_SP_V:SM_SP_U,t,tolerance,raw);
            std::unique_ptr<SmCurve> owner(raw);
            require(status==SM_SUCCESS,"Extract exact native crease isocurve");
            require(raw,"Missing native crease isocurve");
            // Construct the UV segment with the same varying parameter as its
            // exact 3D partner. The pinned SmIsoCurve UV convenience method
            // repeats its U branch and uses the constant interval as its range.
            const SmPoint3d start(d?domain.GetUMin():t,d?t:domain.GetVMin(),0.);
            const SmPoint3d end(d?domain.GetUMax():t,d?t:domain.GetVMax(),0.);
            const auto uv_status=SmBSplineCurve::CreateLineSegment(*brep.GetContext(),2,start,end,uv);
            std::unique_ptr<SmCurve> uv_owner(uv);
            require(uv_status==SM_SUCCESS && uv,"Create native crease UV line");
            require(uv->EditParameterization(raw->GetNaturalInterval(),FALSE)==SM_SUCCESS,"Match crease curve parameterization");
            curves.Add(raw);uv_curves.Add(uv);
            owners.push_back(std::move(owner));owners.push_back(std::move(uv_owner));
        }
        SmTArray<SmFace*> result;
        SmTArray<SmEdge*> edges;
        // Unlike SubdivideFaceAtDiscontinuities, this accepts independently
        // selected kink/tangent cuts, preserves their exact UV curves, and has
        // separate U/V bounds (the pinned convenience method uses U for both).
        require(brep.MergeCurvesOnSurface(*surface,tolerance,curves,&uv_curves,result,edges,FALSE)==SM_SUCCESS,"Split native faces at creases");
        SmTArray<SmFace*> current;
        brep.GetFaces(current);
        require(current.GetSize()<=16384,"Crease output face resource limit");
    }
    SmTArray<SmFace*> result;
    brep.GetFaces(result);
    require(result.GetSize()>=original.GetSize(),"Crease division removed faces");
    size_t output_controls=0;
    for(ULONG f=0;f<result.GetSize();++f){
        output_controls+=controls(result[f]->GetSurface());
        require(output_controls<=2000000,"Crease output control resource limit");
    }
    return result.GetSize()-original.GetSize();
}
