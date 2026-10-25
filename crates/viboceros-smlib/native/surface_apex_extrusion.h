#pragma once
// Included after the bridge's private topology helpers and native owner types.
#include "../../viboceros-opennurbs/native/curve_discontinuity_query.h"
namespace {
void vb_apex_boundary_segments(SmBSplineCurve &profile, bool split_tangents,
    SmTArray<SmBSplineCurve*> &segments) {
    auto *context=SmApiGetOrCreateContext();
    const auto count=profile.GetNumberControlPoints();
    const auto knots=vb_curve_full_knots(profile);
    std::vector<double> controls(4*count),parameters(knots.size());
    for(ULONG i=0;i<count;++i) {
        SmPoint3d point;double weight;
        check(profile.GetControlPoint(SM_CP_EUCLIDIAN_RATIONAL,i,point,weight),"read apex continuity controls");
        controls[4*i]=point.x;controls[4*i+1]=point.y;controls[4*i+2]=point.z;controls[4*i+3]=weight;
    }
    size_t found=0;char error[1024]{};
    const auto status=vibo_curve_discontinuities(profile.GetDegree(),count,controls.data(),
        knots.size(),knots.data(),split_tangents?1:0,std::cos(0.1*std::acos(-1.)/180.),
        &found,parameters.data(),error,sizeof(error));
    require(status==1,error[0]?error:"Apex boundary continuity query failed");
    require(found<100000,"Surface apex boundary segment limit");
    // SMLib's segment constructor copies the input control net before trimming.
    // Bound that work before copying a large net once for every cut.
    require(count<=4*limit/(found+1),"Surface apex boundary split work resource limit");
    const auto domain=profile.GetNaturalInterval();double start=domain.GetMin();
    for(size_t i=0;i<=found;++i) {
        const double end=i<found?parameters[i]:domain.GetMax();
        std::unique_ptr<SmBSplineCurve> segment(new (*context) SmBSplineCurve(profile));
        vb_preserve_curve_basis(*segment,knots);
        SmExtent1d interval(start,end);
        check(segment->Trim(interval,FALSE),"trim apex boundary segment");
        segments.Add(segment.release());start=end;
    }
}
} // namespace

extern "C" int vb_shape_extrude_faces_to_point(const VbSolid *shape, const double apex[3],
    int cap_base, int split_at_tangents, double tolerance, VbSolid **out, char *error, size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(shape && shape->value && out && finite3(apex) && (cap_base==0 || cap_base==1)
            && (split_at_tangents==0 || split_at_tangents==1)
            && std::isfinite(tolerance) && tolerance>0,"Invalid surface apex extrusion input");
        require(!shape->value->IsManifoldSolid(),"Surface apex extrusion requires an open sheet");
        auto *context=SmApiGetOrCreateContext();
        SmTArray<SmFace*> faces;shape->value->GetFaces(faces);
        require(faces.GetSize()>0 && faces.GetSize()<=100000,"Surface apex extrusion face limit");
        const SmPoint3d point(apex[0],apex[1],apex[2]);
        for(ULONG i=0;i<faces.GetSize();++i) {
            SmVector3d normal;
            if(surface_control_plane_normal(*faces[i]->GetSurface(),tolerance,normal)) {
                SmPoint3d base;
                check(faces[i]->GetSurface()->EvaluatePoint(faces[i]->GetSurface()->GetNaturalUVDomain().GetMin(),base),"evaluate planar apex source");
                require(std::abs((point-base).Dot(normal))>tolerance,"Apex must leave the source plane");
            }
        }
        SmTArray<SmEdge*> edges;shape->value->GetEdges(edges);
        size_t controls=0,boundaries=0;
        for(ULONG i=0;i<edges.GetSize();++i)if(edges[i]->IsLamina()) {
            auto *curve=edges[i]->GetCurve();
            require(curve && curve->IsKindOf(SmBSplineCurve_TYPE),"Surface apex extrusion requires NURBS edges");
            const size_t count=static_cast<SmBSplineCurve*>(curve)->GetNumberControlPoints();
            require(count<=limit/2 && controls<=limit/2-count,"Surface apex extrusion control-net resource limit");
            controls+=count;++boundaries;
        }
        require(boundaries>0,"Surface apex extrusion requires a boundary");
        auto owner=std::make_unique<VbSolid>();
        owner->value.reset(new (*context) SmBrep());owner->value->SetTolerance(tolerance,FALSE);
        size_t wall_controls=0,wall_count=0;
        for(ULONG i=0;i<edges.GetSize();++i)if(edges[i]->IsLamina()) {
            auto *original=static_cast<SmBSplineCurve*>(edges[i]->GetCurve());
            std::unique_ptr<SmBSplineCurve> profile(new (*context) SmBSplineCurve(*original));
            vb_preserve_curve_basis(*profile,vb_curve_full_knots(*original));
            auto interval=edges[i]->GetInterval();
            check(profile->Trim(interval,FALSE),"trim apex source edge");
            SmTArray<SmBSplineCurve*> segments;SmObjsDelete<SmBSplineCurve*> cleanup(&segments);
            vb_apex_boundary_segments(*profile, split_at_tangents!=0, segments);
            require(segments.GetSize()>0 && segments.GetSize()<=100000,"Surface apex boundary segment limit");
            if(segments.GetSize()==1 && segments[0]->GetDegree()==profile->GetDegree()
               && segments[0]->GetNumberControlPoints()==profile->GetNumberControlPoints())
                vb_preserve_curve_basis(*segments[0],vb_curve_full_knots(*profile));
            for(ULONG j=0;j<segments.GetSize();++j) {
                auto *section=segments[j];
                const size_t count=section->GetNumberControlPoints();
                require(count<=limit/2 && wall_controls<=limit/2-count,"Surface apex split control-net resource limit");
                wall_controls+=count;
                require(++wall_count<=100000-(cap_base?faces.GetSize():0),"Surface apex output face resource limit");
                auto wall=apex_ruled_surface(*section,point);auto *raw=wall.get();
                SmFace *face=nullptr;
                const auto face_status=owner->value->CreateFaceFromSurface(raw,raw->GetNaturalUVDomain(),face);
                // The native face constructor attaches the surface once it is
                // adopted. Keep ownership if an earlier validation failed.
                if(raw->GetOwner())wall.release();
                check(face_status,"create apex wall topology");require(face,"Missing apex wall face");
            }
        }
        if(cap_base)check(shape->value->CopyFaces(faces,owner->value.get(),nullptr,FALSE),"copy actual apex base caps");
        check(owner->value->StitchAndOrient(),"stitch surface apex extrusion");
        SmTArray<SmFace*> output;owner->value->GetFaces(output);
        require(output.GetSize()>0 && output.GetSize()<=100000,"Surface apex extrusion output face limit");
        require(!cap_base || owner->value->IsManifoldSolid(),"Surface apex extrusion did not close");
        check(SmApiTurnToNurbs(owner->value.get()),"convert surface apex extrusion to NURBS");
        *out=owner.release();
    });
}
