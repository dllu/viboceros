#include "bridge.h"
#include "basis_preservation.h"
#include "knot_refinement.h"
#include <SmApiBrep.h>
#include <SmApiCurves.h>
#include <SmApiGeneral.h>
#include <SmApiIntersectors.h>
#include <SmApiPrimitives.h>
#include <SmApiQueries.h>
#include <SmApiSurfaces.h>
#include <SmBSplineCurve.h>
#include <SmSolutionArray.h>
#include <SmBSplineSurface.h>
#include <NL_Nurbsdef.h>
#include <SmAxis2Placement.h>
#include <SmBrep.h>
#include <SmFace.h>
#include <SmLoop.h>
#include <SmEdge.h>
#include <SmFaceuse.h>
#include <SmRegion.h>
#include <SmShell.h>
#include <SmStitch.h>
#include <SmVertex.h>
#include <SmFilletExecutive.h>
#include <SmCurveFilletExecutive.h>
#include <SmOffsetCurve.h>
#include <SmHCR.h>
#include <SmTopologySweep.h>
#include <SmTransSweepGeometry.h>
#include <SmPoly.h>
#include <SmPrimitiveCreation.h>
#include <SmPointSet.h>
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <vector>

struct VbSolid {
    std::unique_ptr<SmBrep> value;
    // Only fresh imports retain source indices. Mutating operations clear this
    // correspondence before topology changes; copies and Boolean results have none.
    std::vector<SmEdge *> source_edges;
};
struct VbParts {
    std::vector<std::unique_ptr<VbSolid>> values;
};
struct VbCurve {
    std::unique_ptr<SmBSplineCurve> value;
    // The evaluator rejects some tiny charts even after exact knot restoration.
    // A normalized native query chart retains the same imported controls.
    std::unique_ptr<SmBSplineCurve> evaluation;
    size_t degree;
    std::vector<double> points, knots;
};
struct VbCurveIntersections {
    std::vector<VbCurveIntersection> events;
};
struct VbCurveSurfaceIntersections {
    std::vector<VbCurveSurfaceIntersection> events;
};
struct NurbsSnapshotDelete { void operator()(VbSurfaceData *p) const { vb_surface_data_free(p); } };
struct VbCurveList {
    std::vector<std::unique_ptr<VbSurfaceData, NurbsSnapshotDelete>> curves;
};
struct SurfaceIntersectionEntry {
    int kind;
    double point[3]{};
    std::unique_ptr<VbSurfaceData,NurbsSnapshotDelete> curves[3];
};
struct VbSurfaceIntersections { std::vector<SurfaceIntersectionEntry> entries; };
struct VbMesh {
    std::vector<double> points;
    std::vector<uint32_t> triangles;
};

namespace {
std::mutex kernel_mutex;
constexpr size_t limit = 1'000'000;
void require(bool condition, const char *message) {
    if (!condition)
        throw std::runtime_error(message);
}
void check(SmApiStatus status, const char *operation) {
    if (status != SM_SUCCESS)
        throw std::runtime_error(std::string(operation) + " status " + std::to_string(status));
}
template <class Function> int guarded(char *error, size_t capacity, Function function) noexcept {
    try {
        std::lock_guard<std::mutex> lock(kernel_mutex);
        static const bool initialized = [] {
            SmApiCreateContext();
            return true;
        }();
        (void)initialized;
        function();
        return 0;
    } catch (const std::exception &exception) {
        if (error && capacity)
            std::snprintf(error, capacity, "%s", exception.what());
    } catch (...) {
        if (error && capacity)
            std::snprintf(error, capacity, "Unknown SMLib exception");
    }
    return 1;
}
bool finite3(const double *p) {
    return p && std::isfinite(p[0]) && std::isfinite(p[1]) && std::isfinite(p[2]);
}

std::unique_ptr<SmBSplineCurve> copy_curve(const VbCurve &curve) {
    SmCurve *copied = nullptr;
    const auto status = curve.value->Copy(*SmApiGetOrCreateContext(), copied);
    std::unique_ptr<SmCurve> copy_owner(copied);
    check(status, "copy modeling curve");
    require(copied && copied->IsKindOf(SmBSplineCurve_TYPE), "Missing NURBS curve copy");
    vb_preserve_curve_basis(*static_cast<SmBSplineCurve *>(copied), curve.knots);
    copy_owner.release();
    return std::unique_ptr<SmBSplineCurve>(static_cast<SmBSplineCurve *>(copied));
}

SmPoint3d curve_start(const VbCurve &curve) {
    SmPoint3d result;
    const auto *query = curve.evaluation ? curve.evaluation.get() : curve.value.get();
    check(query->EvaluatePoint(query->GetNaturalInterval().GetMin(), result), "evaluate modeling curve start");
    return result;
}

// GetCanonical groups knots at an absolute tolerance. Expand the chart by an
// exact binary factor before producers that use that query; preserve the input
// basis and controls with the existing data setter, then restore the result.
std::unique_ptr<SmBSplineCurve> canonical_working_curve(const VbCurve &curve) {
    long double gap = INFINITY;
    for (size_t i = 1; i < curve.knots.size(); ++i)
        if (curve.knots[i] > curve.knots[i - 1])
            gap = std::min(gap, static_cast<long double>(curve.knots[i]) - curve.knots[i - 1]);
    if (gap > 64 * SM_EFF_ZERO) return nullptr;
    auto copy = copy_curve(curve);
    {
        const int shift = std::ilogb(64 * SM_EFF_ZERO) - std::ilogb(gap) + 1;
        std::vector<double> knots;
        for (double k : curve.knots) {
            const double scaled = std::ldexp(k, shift);
            require(std::isfinite(scaled), "Canonical working chart exceeds finite knot range");
            knots.push_back(scaled);
        }
        vb_preserve_curve_basis(*copy, knots);
    }
    return copy;
}

std::unique_ptr<VbSolid> capped_surface(std::unique_ptr<SmBSplineSurface> surface,
                                       int ends, double tolerance) {
    auto owner = std::make_unique<VbSolid>();
    owner->value.reset(new (*SmApiGetOrCreateContext()) SmBrep());
    owner->value->SetTolerance(tolerance, FALSE);
    auto *raw = surface.release();
    const auto status = owner->value->CreateEndCapsFromSurface(raw, ends);
    if (status != SM_SUCCESS && !raw->GetOwner()) surface.reset(raw);
    check(status, "create swept solid caps");
    check(owner->value->StitchAndOrient(), "stitch swept solid");
    require(owner->value->IsManifoldSolid(), "Sweep did not produce a manifold solid");
    check(SmApiTurnToNurbs(owner->value.get()), "convert exact swept solid to NURBS");
    return owner;
}

std::unique_ptr<SmBSplineSurface> translational_sweep(const VbCurve &profile, const VbCurve &path) {
    require(profile.points.size() / 4 <= limit / (path.points.size() / 4),
            "Translational sweep control-net resource limit");
    bool nonconstant = false;
    for (size_t i = 4; i < path.points.size(); i += 4)
        for (size_t axis = 0; axis < 3; ++axis)
            nonconstant = nonconstant || path.points[i + axis] != path.points[axis];
    require(nonconstant, "Constant translational sweep path");
    const auto start = curve_start(path);
    auto shifted = copy_curve(path);
    const SmAxis2Placement translation(SmPoint3d(-start.x, -start.y, -start.z),
                                      SmVector3d(1, 0, 0), SmVector3d(0, 1, 0));
    check(shifted->Transform(translation), "translate sweep path to its start");
    SmBSplineSurface *raw = nullptr;
    const auto status = SmBSplineSurface::CreateSweepSurface(*SmApiGetOrCreateContext(),
        profile.value.get(), shifted.get(), nullptr, TRUE, 0, raw);
    std::unique_ptr<SmBSplineSurface> result(raw);
    check(status, "create exact translational sweep");
    require(raw, "Missing translational sweep surface");
    vb_preserve_surface_basis(*raw, {profile.knots, path.knots});
    return result;
}

std::unique_ptr<SmBSplineSurface> apex_ruled_surface(const SmBSplineCurve &curve,const SmPoint3d &point) {
    auto *context=SmApiGetOrCreateContext();
    const size_t count=curve.GetNumberControlPoints();
    require(count>0 && count<=limit/2,"Apex ruled-surface control-net resource limit");
    std::unique_ptr<SmBSplineCurve> collapsed(new (*context) SmBSplineCurve(curve));
    vb_preserve_curve_basis(*collapsed,vb_curve_full_knots(curve));
    for(ULONG i=0;i<count;++i) {
        SmPoint3d control;double weight;
        check(curve.GetControlPoint(SM_CP_EUCLIDIAN_RATIONAL,i,control,weight),"read apex profile weight");
        check(collapsed->SetControlPoint(SM_CP_EUCLIDIAN_RATIONAL,i,point,weight),"collapse apex boundary");
    }
    SmBSplineSurface *raw=nullptr;
    const auto status=SmBSplineSurface::CreateRuledSurface(*context,curve,*collapsed,SM_SP_U,raw);
    std::unique_ptr<SmBSplineSurface> result(raw);
    check(status,"create apex ruled surface");require(raw,"Missing apex ruled surface");
    return result;
}
std::unique_ptr<SmBSplineSurface> apex_surface(const VbCurve &curve, const double apex[3]) {
    require(finite3(apex), "Invalid apex coordinates");
    const SmPoint3d point(apex[0],apex[1],apex[2]);
    auto result=apex_ruled_surface(*curve.value,point);
    auto *raw=result.get();
    vb_preserve_surface_basis(*raw, {std::vector<double>{0, 0, 1, 1}, curve.knots});
    const auto start = curve_start(curve);
    const double length = start.DistanceBetween(point);
    require(std::isfinite(length) && length > 0, "Apex coincides with the profile start");
    const auto domain = raw->GetNaturalUVDomain();
    check(raw->Reparameterize(SmExtent2d(0, domain.GetMin().y, length, domain.GetMax().y)),
          "set apex surface parameter domain");
    vb_preserve_surface_basis(*raw, {std::vector<double>{0, 0, length, length}, curve.knots});
    return result;
}

// Inputs are already oriented by the shared command policy. NLib compatibility
// and Coons construction can mutate curve caches, so always use private copies.
std::unique_ptr<SmBSplineSurface> edge_surface(const VbCurve *const *curves, size_t count,
                                              double tolerance) {
    require(curves && count >= 2 && count <= 4 && std::isfinite(tolerance) && tolerance > 0,
            "Invalid edge surface inputs");
    std::vector<std::unique_ptr<SmBSplineCurve>> copies;
    for (size_t i = 0; i < count; ++i) {
        require(curves[i], "Missing edge surface boundary");
        require(curves[i]->points.size() / 4 <= 512, "Edge boundary control resource limit");
        copies.push_back(copy_curve(*curves[i]));
    }
    auto reverse = [&](size_t i) {
        const auto original = copies[i]->GetNaturalInterval();
        SmExtent1d interval;
        check(copies[i]->ReverseParameterization(original, interval),
              "reverse edge surface boundary");
        // Native reversal retains its interval. Rhino's boundary convention
        // negates the chart, including the domain chosen for opposite pairs.
        check(copies[i]->ScaleKnotVector(-original.GetMax(), -original.GetMin()),
              "set reversed edge boundary parameter domain");
    };
    auto match_domains = [&](SmBSplineCurve &a, SmBSplineCurve &b) {
        const auto x = a.GetNaturalInterval(), y = b.GetNaturalInterval();
        const auto target = x.GetMax() - x.GetMin() >= y.GetMax() - y.GetMin() ? x : y;
        check(a.ScaleKnotVector(target.GetMin(), target.GetMax()), "align edge boundary domain");
        check(b.ScaleKnotVector(target.GetMin(), target.GetMax()), "align edge boundary domain");
    };
    std::unique_ptr<SmBSplineSurface> result;
    if (count == 2) {
        match_domains(*copies[0], *copies[1]);
        result = vb_ruled_surface(*copies[0], *copies[1]);
    } else {
        SmBSplineCurve *west, *east, *south, *north;
        std::unique_ptr<SmBSplineCurve> collapsed;
        if (count == 3) {
            reverse(1);
            west = copies[2].get(); east = copies[1].get(); north = copies[0].get();
            collapsed = copy_curve(*curves[0]);
            const auto apex = curve_start(*curves[2]);
            for (size_t i = 0; i < curves[0]->points.size() / 4; ++i)
                check(collapsed->SetControlPoint(SM_CP_EUCLIDIAN_RATIONAL, i, apex,
                                                curves[0]->points[4 * i + 3]),
                      "collapse triangular edge surface boundary");
            south = collapsed.get();
        } else {
            reverse(2); reverse(3);
            west = copies[0].get(); east = copies[2].get();
            north = copies[1].get(); south = copies[3].get();
        }
        match_domains(*west, *east);
        match_domains(*north, *south);
        result = vb_coons_surface({west, east, south, north});
    }
    auto *raw = result.get();
    require(raw, "Missing native edge surface");
    if (count > 2) {
        // The shared payload stores finite Euclidean controls. Coons blending
        // can create a valid homogeneous control at infinity; native degree
        // elevation can supply an equivalent finite-control representation.
        for (int attempt = 0;; ++attempt) {
            if (!raw->IsRational()) break;
            const auto knots = vb_surface_full_knots(*raw);
            const auto nu = knots[0].size() - raw->GetDegree(SM_SP_U) - 1;
            const auto nv = knots[1].size() - raw->GetDegree(SM_SP_V) - 1;
            require(nu && nv && nu <= limit / nv, "Edge surface control-net resource limit");
            std::vector<double> controls(nu * nv * 4);
            check(raw->GetControlPointsExpert(SM_CP_HOMOGENEOUS_RATIONAL, 0, nu - 1,
                                              0, nv - 1, nv * 4, 4, controls.data()),
                  "inspect native edge surface homogeneous controls");
            bool has_infinite_control = false;
            for (size_t i = 0; i < controls.size(); i += 4) {
                require(std::isfinite(controls[i]) && std::isfinite(controls[i + 1]) &&
                            std::isfinite(controls[i + 2]) && std::isfinite(controls[i + 3]),
                        "Nonfinite edge surface homogeneous control");
                has_infinite_control = has_infinite_control || controls[i + 3] == 0;
            }
            if (!has_infinite_control) break;
            require(attempt < 4 && raw->GetDegree(SM_SP_V) < NL_MAXDEG,
                    "No finite-control native edge surface representation");
            check(raw->DegreeElevate(SM_SP_V, raw->GetDegree(SM_SP_V) + 1),
                  "elevate native edge surface projective controls");
        }
        result = vb_swapped_surface_uv(*raw);
    }
    return result;
}
} // namespace

extern "C" int vb_curves_edge_surface(const VbCurve *const *curves, size_t count, double tolerance,
                                       VbSurfaceData **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(out, "Missing edge surface output");
        auto surface = edge_surface(curves, count, tolerance);
        *out = vb_export_surface(*surface);
    });
}

extern "C" int vb_nurbs_insert_knot(const VbNurbsData *source, int axis, double parameter,
                                     size_t target, VbSurfaceData **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(source && out, "Missing knot refinement payload");
        *out = vb_refine_nurbs(*source, axis, parameter, target);
    });
}

extern "C" int vb_nurbs_trim_curve(const VbNurbsData *source,double start,double end,
                                   VbSurfaceData **out,char *error,size_t capacity) {
    if(out) *out=nullptr;
    return guarded(error,capacity,[&] {
        require(source && out,"Missing curve trim payload");
        *out=vb_trim_nurbs_curve(*source,start,end);
    });
}

extern "C" int vb_nurbs_trim_surface_axis(const VbNurbsData *source,int axis,double start,double end,
                                          VbSurfaceData **out,char *error,size_t capacity) {
    if(out) *out=nullptr;
    return guarded(error,capacity,[&] {
        require(source && out,"Missing surface strip payload");
        *out=vb_trim_nurbs_surface_axis(*source,axis,start,end);
    });
}

extern "C" int vb_nurbs_extend_curve(const VbNurbsData *source,double start,double end,
                                     VbSurfaceData **out,char *error,size_t capacity) {
    if(out) *out=nullptr;
    return guarded(error,capacity,[&] {
        require(source && out,"Missing curve extension payload");
        *out=vb_extend_nurbs_curve(*source,start,end);
    });
}

extern "C" int vb_nurbs_surface_isocurve(const VbNurbsData *source,int varying,double parameter,
                                         VbSurfaceData **out,char *error,size_t capacity) {
    if(out) *out=nullptr;
    return guarded(error,capacity,[&] {
        require(source && out,"Missing isocurve payload");
        *out=vb_surface_isocurve(*source,varying,parameter);
    });
}

extern "C" int vb_interpolation_solve(const VbInterpolationSystem *input,double *output,size_t output_count,char *error,size_t capacity) {
    return guarded(error,capacity,[&] {
        require(input && output,"Missing interpolation payload");
        vb_solve_interpolation(*input,output,output_count);
    });
}

extern "C" int vb_nurbs_change_degree(const VbNurbsData *source, int axis, size_t degree,
                                       int deformable, const double *knots, size_t knot_count,
                                       VbSurfaceData **out, char *error, size_t capacity) {
    if (out) *out=nullptr;
    return guarded(error,capacity,[&] {
        require(source && out && (deformable==0 || deformable==1), "Invalid degree-change payload");
        *out=vb_degree_nurbs(*source,axis,degree,deformable!=0,knots,knot_count);
    });
}

extern "C" int vb_nurbs_fit_basis(const VbNurbsData *source, int axis, const double *knots,
                                 size_t knot_count, VbSurfaceData **out, char *error, size_t capacity) {
    if(out) *out=nullptr;
    return guarded(error,capacity,[&] {
        require(source && out,"Invalid basis-fitting payload");
        *out=vb_fit_nurbs(*source,axis,knots,knot_count);
    });
}

extern "C" int vb_nurbs_bezier(const VbNurbsData *source,int direction,VbSurfaceData **out,
                                size_t count,int *control_error,char *error,size_t capacity) {
    if(control_error) *control_error=0;
    return guarded(error,capacity,[&] {
        require(source && out && control_error && count && count<=1'000'000,"Invalid Bezier output buffers");
        std::fill_n(out,count,nullptr);
        try { vb_bezier_nurbs(*source,direction,out,count); }
        catch(const VbUnrepresentableNurbsControl &) { *control_error=1;throw; }
    });
}

extern "C" int vb_shape_edge_surface(const VbCurve *const *curves, size_t count, double tolerance,
                                      VbSolid **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(out, "Missing edge surface shape output");
        auto surface = edge_surface(curves, count, tolerance);
        auto owner = std::make_unique<VbSolid>();
        owner->value.reset(new (*SmApiGetOrCreateContext()) SmBrep());
        owner->value->SetTolerance(tolerance, FALSE);
        auto *raw = surface.release();
        SmFace *face = nullptr;
        const auto status = owner->value->CreateFaceFromSurface(raw, raw->GetNaturalUVDomain(), face);
        if (status != SM_SUCCESS && !raw->GetOwner()) surface.reset(raw);
        check(status, "create native edge surface face");
        require(face, "Missing native edge surface face");
        check(SmApiTurnToNurbs(owner->value.get()), "convert native edge surface topology");
        *out = owner.release();
    });
}

extern "C" int vb_solid_primitive(int kind, const double origin[3], const double size[3],
                                  VbSolid **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(out && finite3(origin) && finite3(size), "Invalid primitive coordinates");
        require(size[0] > 0 && (kind == 1 || size[1] > 0) && (kind != 0 || size[2] > 0),
                "Invalid primitive dimensions");
        auto owner = std::make_unique<VbSolid>();
        SmBrep *result = nullptr;
        const SmVector3d point(origin[0], origin[1], origin[2]);
        SmApiStatus status;
        switch (kind) {
        case 0:
            status = SmApiCreateBox(point, size[0], size[1], size[2], result);
            break;
        case 1:
            status = SmApiCreateSphere(point, size[0], result);
            break;
        case 2:
            status = SmApiCreateCylinder(point, size[0], size[1], result);
            break;
        default:
            throw std::runtime_error("Unknown primitive");
        }
        owner->value.reset(result);
        check(status, "create primitive");
        require(result, "Missing primitive result");
        *out = owner.release();
    });
}

extern "C" int vb_solid_boolean(const VbSolid *a, const VbSolid *b, int operation, int options, VbSolid **out,
                                char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(a && b && out && operation >= 0 && operation <= 2 && options>=0 && options<=3,
                "Invalid Boolean operands or operation");
        auto owner = std::make_unique<VbSolid>();
        SmBrep *raw_a = nullptr;
        SmBrep *raw_b = nullptr;
        const auto copy_a = SmApiBrepCopy(a->value.get(), raw_a);
        owner->value.reset(raw_a);
        check(copy_a, "copy A");
        const auto copy_b = SmApiBrepCopy(b->value.get(), raw_b);
        std::unique_ptr<SmBrep> cutter(raw_b);
        check(copy_b, "copy B");
        SmBrep *result = nullptr;
        // Bit 0 selects topology cleanup; bit 1 selects the native
        // cookie-cutter mode used to classify original boundary coverage.
        const auto status = options!=1 ? SmApiBooleanWithOptions(raw_a, raw_b,
                            operation==0 ? SM_BO_UNION : operation==1 ? SM_BO_INTERSECTION : SM_BO_DIFFERENCE,
                            (options&2)!=0,FALSE,(options&1)!=0,FALSE,FALSE,result)
                            : operation == 0   ? SmApiBooleanUnion(raw_a, raw_b, result)
                            : operation == 1 ? SmApiBooleanIntersection(raw_a, raw_b, result)
                                             : SmApiBooleanDifference(raw_a, raw_b, result);
        if (status == SM_SUCCESS)
            cutter.release();
        check(status, "Boolean");
        require(result == raw_a, "Unexpected Boolean ownership");
        *out = owner.release();
    });
}

extern "C" int vb_solid_properties(const VbSolid *solid, double accuracy, double *volume,
                                   double bounds[6], int *manifold, char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(solid && volume && bounds && manifold && std::isfinite(accuracy) &&
                    accuracy >= 1e-8 && accuracy <= 0.1,
                "Invalid property query");
        SmBoolean closed = FALSE;
        check(SmApiBrepIsManifoldSolid(solid->value.get(), closed), "manifold");
        SmPoint3d lo, hi;
        check(SmApiBrepBoundingBox(solid->value.get(), TRUE, lo, hi), "bounds");
        double v = 0;
        check(SmApiBrepComputeVolume(solid->value.get(), accuracy, v), "volume");
        require(std::isfinite(v), "Nonfinite volume");
        for (int i = 0; i < 3; ++i) {
            require(std::isfinite(lo[i]) && std::isfinite(hi[i]) && lo[i] <= hi[i],
                    "Invalid bounds");
            bounds[i] = lo[i];
            bounds[i + 3] = hi[i];
        }
        *volume = v;
        *manifold = closed == TRUE;
    });
}

extern "C" int vb_solid_volume(const VbSolid *solid, double accuracy, double *volume,
                               char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(solid && volume && std::isfinite(accuracy) && accuracy >= 1e-12 && accuracy <= 0.1,
                "Invalid volume query");
        double result = 0;
        SmTArray<SmFace *> faces;
        solid->value->GetFaces(faces);
        if (faces.GetSize() != 0) {
            require(solid->value->IsManifoldSolid(), "Volume requires a manifold solid");
            // SM_API clamps accuracy to 1e-8, which is too coarse for branch
            // conservation at the document tolerance. Use the public kernel
            // integration entry directly, with the same local computation origin.
            SmExtent3d bounds;
            check(solid->value->CalculateBoundingBox(bounds), "volume origin");
            const auto size = bounds.GetSize();
            const double scale = std::max({size.x, size.y, size.z});
            const double estimated_area = scale * scale / 100.0;
            // Request only volume from the upstream face integrator. The B-rep
            // property query also integrates area and 24 moment components.
            // Region flags supply material/void ownership, including cavities.
            for (ULONG i = 0; i < faces.GetSize(); ++i) {
                auto *first = faces[i]->GetUpwardFaceuse();
                auto *second = first->GetMate();
                const bool first_void = first->GetShell()->GetRegion()->IsVoid();
                const bool second_void = second->GetShell()->GetRegion()->IsVoid();
                if (first_void == second_void) continue;
                const auto *material = first_void ? second : first;
                double area = 0, delta = 0;
                SmTArray<SmVector3d> moments;
                check(faces[i]->ComputePreciseProperties(material->GetOrientation(), accuracy,
                      bounds.GetMid(), estimated_area, 0.0, area, delta, moments, SM_PPF_VOLUME),
                      "volume");
                result += delta;
            }
        }
        require(std::isfinite(result), "Nonfinite volume");
        *volume = result;
    });
}

extern "C" void vb_solid_free(VbSolid *solid) {
    std::lock_guard<std::mutex> lock(kernel_mutex);
    delete solid;
}

extern "C" int vb_solid_census(const VbSolid *solid, size_t counts[2], char *error,
                               size_t capacity) {
    return guarded(error, capacity, [&] {
        require(solid && counts, "Invalid material census");
        long material = 0, voids = 0;
        check(SmApiBrepMaterialCensus(solid->value.get(), material, voids), "material census");
        require(material >= 0 && voids >= 0, "Invalid material counts");
        counts[0] = material;
        counts[1] = voids;
    });
}

extern "C" int vb_solid_brep(const VbSolid *solid, VbBrep **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(solid && out, "Invalid solid export");
        *out = vb_export_brep(*solid->value);
    });
}

extern "C" int vb_shape_brep(const VbSolid *shape, double approximation, VbBrep **out,
                              char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(shape && out && std::isfinite(approximation) && approximation > 0,
                "Invalid shape export");
        *out = vb_export_brep(*shape->value, approximation);
    });
}

extern "C" int vb_solid_from_brep(const VbBrepView *view, double tolerance,
                                  const size_t *components, size_t component_count,
                                  const int *inward, VbSolid **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(view && out, "Invalid B-rep import");
        auto owner = std::make_unique<VbSolid>();
        owner->value.reset(vb_import_brep(*view, tolerance, components, component_count, inward));
        *out = owner.release();
    });
}

extern "C" int vb_sheet_from_brep(const VbBrepView *view, double tolerance,
                                  const size_t *components, size_t component_count,
                                  VbSolid **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(view && out && component_count && component_count <= 128,
                "Invalid sheet import");
        std::vector<int> inward(component_count, 0);
        auto owner = std::make_unique<VbSolid>();
        owner->value.reset(vb_import_brep(*view, tolerance, components, component_count,
                                         inward.data(), nullptr, nullptr, true));
        *out = owner.release();
    });
}

extern "C" int vb_shape_offset(const VbSolid *shape, double distance, int wall, double accuracy,
                                VbSolid **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(shape && out && std::isfinite(distance) && distance != 0 &&
                    (wall == 0 || wall == 1) && std::isfinite(accuracy) && accuracy > SM_EFF_ZERO,
                "Invalid shape offset or accuracy");
        auto owner = std::make_unique<VbSolid>();
        std::unique_ptr<SmBrep> working(vb_copy_brep(*shape->value));
        const auto previous_tolerance = working->GetTolerance();
        working->SetTolerance(accuracy);
        struct RestoreTolerance {
            SmBrep *brep;
            double previous;
            ~RestoreTolerance() { brep->SetTolerance(previous); }
        } restore{working.get(), previous_tolerance};
        SmBrep *raw = nullptr;
        if (wall) {
            SmTArray<const SmFace *> faces;
            const auto status = SmApiShellBrepFull(working.get(), distance,
                                                  TRUE, TRUE, TRUE, faces, raw);
            owner->value.reset(raw);
            check(status, "shell boundary representation");
        } else {
            const auto status = SmApiOffsetBrepFull(working.get(), distance, TRUE, TRUE, raw);
            require(raw != working.get(), "Offset returned its input instead of a new result");
            owner->value.reset(raw);
            check(status, "offset boundary representation");
        }
        require(owner->value != nullptr, "Missing offset result");
        *out = owner.release();
    });
}

extern "C" int vb_shape_face_count(const VbSolid *shape, size_t *count, char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(shape && count, "Invalid face count query");
        SmTArray<SmFace *> faces;
        shape->value->GetFaces(faces);
        *count = faces.GetSize();
    });
}

extern "C" int vb_shape_bounds(const VbSolid *shape, double bounds[6], char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(shape && bounds, "Invalid shape bounds query");
        SmPoint3d lo, hi;
        check(SmApiBrepBoundingBox(shape->value.get(), TRUE, lo, hi), "tight shape bounds");
        for (int i = 0; i < 3; ++i) {
            require(std::isfinite(lo[i]) && std::isfinite(hi[i]) && lo[i] <= hi[i], "Invalid shape bounds");
            bounds[i] = lo[i]; bounds[i + 3] = hi[i];
        }
    });
}

extern "C" int vb_shape_stitch(const VbSolid *shape, double distance, size_t *face_origins,
                                size_t face_capacity, size_t *face_count,
                                size_t *stitched_edges, double gaps[2], VbSolid **out,
                                char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(shape && out && face_origins && face_count && stitched_edges && gaps &&
                    std::isfinite(distance) && distance > 0,
                "Invalid stitching arguments");
        auto owner = std::make_unique<VbSolid>();
        owner->value.reset(vb_copy_brep(*shape->value));
        SmTArray<SmFace *> original, faces;
        owner->value->GetFaces(original);
        require(original.GetSize() <= face_capacity, "Insufficient stitching face buffer");
        for (ULONG i = 0; i < original.GetSize(); ++i)
            original[i]->SetUserIndex1(i + 1);
        ULONG stitched = 0, lamina = 0;
        double vertex_gap = 0, edge_gap = 0;
        SmStitchCallback callback;
        // Join accepts open sheets and coincident lamina faces. Region nesting
        // during edge gluing can fail on those inputs; let the orientation pass
        // handle the resulting connected sheets and closed shells instead.
        SmStitch executive(callback, distance, FALSE);
        executive.m_bSqueezeSmallEdges = FALSE;
        executive.m_bSplitEdgesWithVertices = TRUE;
        executive.m_bMakingManifoldSolid = TRUE;
        executive.m_bRemoveLaminarSlivers = FALSE;
        executive.m_bFastEdgeCompare = FALSE;
        executive.m_bIgnoreProblems = TRUE;
        check(executive.DoStitching(owner->value.get(), nullptr, nullptr, stitched, lamina,
                                    vertex_gap, edge_gap), "stitch boundaries");
        SmBoolean maybe_open = FALSE;
        check(owner->value->OrientTrimmedSurfaces(TRUE, maybe_open, lamina > 0),
              "orient stitched boundaries");
        owner->value->GetFaces(faces);
        require(faces.GetSize() <= face_capacity, "Stitching exceeded face buffer");
        for (ULONG i = 0; i < faces.GetSize(); ++i) {
            const auto index = faces[i]->GetUserIndex1();
            require(index > 0 && index <= original.GetSize(), "Stitching lost source face identity");
            face_origins[i] = index - 1;
        }
        require(std::isfinite(vertex_gap) && std::isfinite(edge_gap), "Invalid stitching gaps");
        *face_count = faces.GetSize();
        *stitched_edges = stitched;
        gaps[0] = vertex_gap;
        gaps[1] = edge_gap;
        *out = owner.release();
    });
}

extern "C" int vb_solid_from_brep_plan(const VbBrepView *view, double tolerance,
                                      const size_t *components, size_t component_count,
                                      const int *inward, size_t *parents, size_t parent_capacity,
                                      VbSolid **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(view && out && parents && component_count && component_count <= 128 &&
                    parent_capacity >= component_count,
                "Invalid shell plan buffer");
        auto owner = std::make_unique<VbSolid>();
        std::vector<size_t> plan;
        owner->value.reset(
            vb_import_brep(*view, tolerance, components, component_count, inward, &plan,
                           &owner->source_edges));
        require(plan.size() == component_count, "Incomplete native shell plan");
        std::copy(plan.begin(), plan.end(), parents);
        *out = owner.release();
    });
}

extern "C" int vb_imported_solid_fillet_edges(VbSolid *solid, const size_t *indices, size_t count,
                                              double radius, double accuracy, char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(solid && indices && count && count <= 100000 && std::isfinite(radius) && radius > 0,
                "Invalid fillet parameters");
        require(std::isfinite(accuracy) && accuracy > SM_EFF_ZERO,
                "Fillet accuracy is below the native numerical floor");
        require(!solid->source_edges.empty(), "Fillet requires fresh source edge identities");
        std::vector<bool> seen(solid->source_edges.size(), false);
        SmTArray<SmEdge *> edges;
        for (size_t i = 0; i < count; ++i) {
            require(indices[i] < seen.size(), "Fillet source edge index is out of range");
            if (!seen[indices[i]]) {
                seen[indices[i]] = true;
                edges.Add(solid->source_edges[indices[i]]);
            }
        }
        vb_validate_boundary_intersections(*solid->value,10*accuracy);
        solid->source_edges.clear();
        SmFilletExecutive executive(*SmApiGetOrCreateContext(), solid->value.get());
        check(executive.SetFilletParameters(solid->value.get(), edges, SM_FSG_CIRCULAR,
              SM_FS_CONST_RADIUS, radius, nullptr, accuracy, -1.0, -1.0, std::min(accuracy,1e-10), 1, 1.0),
              "configure precise edge fillet");
        executive.SetDoGlobalMerge(TRUE);
        check(executive.DoFillet(), "fillet source edges");
        require(executive.GetFilletErrorInfo()->GetErrorCode() == SM_FILERR_OK,
                "Fillet executive reported an incomplete or failed operation");
        check(solid->value->StitchAndOrient(), "orient fillet output");
        require(solid->value->IsManifoldSolid(), "Fillet output is not a manifold solid");
        vb_validate_boundary_intersections(*solid->value,10*accuracy);
    });
}

extern "C" int vb_solid_validate_boundary(const VbSolid *solid, double tolerance,
                                           char *error, size_t capacity) {
    return guarded(error,capacity,[&] {
        require(solid,"Invalid boundary validation handle");
        vb_validate_boundary_intersections(*solid->value,tolerance);
    });
}

extern "C" int vb_solid_validate_endpoints(const VbSolid *solid, double tolerance,
                                            char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(solid && std::isfinite(tolerance) && tolerance > 0,
                "Invalid native endpoint validation input");
        SmTArray<SmEdge *> edges;
        solid->value->GetEdges(edges);
        require(edges.GetSize() <= limit, "Native endpoint validation work limit");
        for (ULONG i = 0; i < edges.GetSize(); ++i) {
            const auto *edge = edges[i];
            const auto interval = edge->GetInterval();
            for (int end = 0; end < 2; ++end) {
                SmPoint3d endpoint;
                check(edge->GetCurve()->EvaluatePoint(end ? interval.GetMax() : interval.GetMin(), endpoint),
                      "evaluate native edge endpoint");
                const auto *vertex = end ? edge->GetEndVertex() : edge->GetStartVertex();
                const double gap = endpoint.DistanceBetween(vertex->GetPoint());
                const double vertex_radius = vertex->GetTolerance(), edge_radius = edge->GetTolerance();
                const double radius = vertex_radius + edge_radius;
                require(std::isfinite(gap) && std::isfinite(radius) &&
                            vertex_radius >= 0 && edge_radius >= 0,
                        "Nonfinite native endpoint uncertainty");
                if (gap > std::max(tolerance, radius)) {
                    char detail[256];
                    std::snprintf(detail, sizeof(detail),
                        "Native edge %lu endpoint %d exceeds its native vertex/edge zone radii: gap %.17g, radius %.17g",
                        static_cast<unsigned long>(i), end, gap, radius);
                    throw std::runtime_error(detail);
                }
            }
        }
    });
}

extern "C" int vb_solid_mesh(const VbSolid *solid, const double quality[3], VbMesh **out,
                             char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(solid && out && finite3(quality) && quality[0] > 0 && quality[1] > 0 &&
                    quality[1] <= 180 && quality[2] > 0 && quality[2] <= 180,
                "Invalid tessellation controls");
        SmTessellationParams params;
        params.dChordHeightTol = quality[0];
        params.dCurveAngleTolDeg = quality[1];
        params.dSurfaceAngleTolDeg = quality[2];
        SmPolyBrep *raw = nullptr;
        const auto status = SmApiTessellate(solid->value.get(), raw, params);
        std::unique_ptr<SmPolyBrep> mesh(raw);
        check(status, "tessellate");
        require(raw, "Missing tessellation result");
        auto result = std::make_unique<VbMesh>();
        SmTArray<SmPolyVertex *> vertices;
        SmTArray<SmPolyFace *> faces;
        mesh->GetPolyVertices(vertices);
        mesh->GetPolyFaces(faces);
        require(vertices.GetSize() <= limit && faces.GetSize() <= limit, "Mesh resource limit");
        std::unordered_map<const SmPolyVertex *, uint32_t> indices;
        for (ULONG i = 0; i < vertices.GetSize(); ++i) {
            indices.emplace(vertices[i], static_cast<uint32_t>(i));
            const auto &p = vertices[i]->GetPoint();
            require(std::isfinite(p.x) && std::isfinite(p.y) && std::isfinite(p.z),
                    "Nonfinite mesh vertex");
            result->points.insert(result->points.end(), {p.x, p.y, p.z});
        }
        for (ULONG i = 0; i < faces.GetSize(); ++i) {
            auto *loop = faces[i]->GetOuterPolyLoop();
            require(loop, "Missing mesh face loop");
            SmTArray<SmPolyEdge *> edges;
            loop->GetPolyEdges(edges);
            require(edges.GetSize() == 3, "Expected triangular tessellation face");
            for (ULONG j = 0; j < 3; ++j)
                result->triangles.push_back(indices.at(edges[j]->GetStartPolyVertex()));
        }
        *out = result.release();
    });
}

extern "C" int vb_mesh_sizes(const VbMesh *mesh, size_t *vertices, size_t *triangles) {
    if (!mesh || !vertices || !triangles)
        return 1;
    *vertices = mesh->points.size() / 3;
    *triangles = mesh->triangles.size() / 3;
    return 0;
}
extern "C" int vb_mesh_copy(const VbMesh *mesh, double *vertices, size_t vertex_count,
                            uint32_t *triangles, size_t triangle_count) {
    if (!mesh || vertex_count != mesh->points.size() / 3 ||
        triangle_count != mesh->triangles.size() / 3 || !vertices || !triangles)
        return 1;
    std::copy(mesh->points.begin(), mesh->points.end(), vertices);
    std::copy(mesh->triangles.begin(), mesh->triangles.end(), triangles);
    return 0;
}
extern "C" void vb_mesh_free(VbMesh *mesh) { delete mesh; }

extern "C" int vb_curve_create(size_t degree, const double *points_xyzw, size_t point_count,
                               const double *knots, size_t knot_count, VbCurve **out, char *error,
                               size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(out && points_xyzw && knots && degree > 0 && degree <= NL_MAXDEG && point_count > degree &&
                    point_count <= limit && knot_count == point_count + degree + 1,
                "Invalid NURBS dimensions");
        SmTArray<SmPoint3d> points;
        SmTArray<double> weights, unique;
        SmTArray<ULONG> multiplicities;
        bool rational = false;
        for (size_t i = 0; i < point_count; ++i) {
            const auto *p = points_xyzw + i * 4;
            require(finite3(p) && std::isfinite(p[3]) && p[3] > 0,
                    "Native NURBS require positive finite weights");
            points.Add(SmPoint3d(p[0], p[1], p[2]));
            weights.Add(p[3]);
            rational = rational || p[3] != points_xyzw[3];
        }
        for (size_t i = 0; i < knot_count; ++i) {
            require(std::isfinite(knots[i]) && (i == 0 || knots[i] >= knots[i - 1]),
                    "Invalid knots");
            if (i == 0 || knots[i] != knots[i - 1]) {
                unique.Add(knots[i]);
                multiplicities.Add(1);
            } else
                ++multiplicities[multiplicities.GetSize() - 1];
        }
        const double start = knots[degree], end = knots[point_count];
        require(start < end, "Empty active NURBS interval");
        for (ULONG i = 0; i < unique.GetSize(); ++i)
            if (unique[i] > start && unique[i] < end)
                require(multiplicities[i] <= degree,
                        "Native bridge does not support discontinuous interior knots");
        auto owner = std::make_unique<VbCurve>();
        SmBSplineCurve *raw = nullptr;
        // A common weight is an irrelevant polynomial gauge. The existing
        // optional-weight form avoids the IGES validator's explicit-weight limits.
        const auto status = SmApiCreateCanonicalCurve(points, unique, multiplicities,
                                                      rational ? &weights : nullptr,
                                                      degree, SM_CF_UNSPECIFIED, raw);
        owner->value.reset(raw);
        check(status, "create canonical curve");
        require(raw, "Missing NURBS result");
        // CreateCanonical's existing IGES routine clamps the supplied full
        // NURBS to its active interval. Do not duplicate that operation in Rust.
        owner->degree = raw->GetDegree();
        std::vector<double> expected_knots(degree + 1, start);
        for (size_t i = 0; i < knot_count; ++i)
            if (knots[i] > start && knots[i] < end) expected_knots.push_back(knots[i]);
        expected_knots.insert(expected_knots.end(), degree + 1, end);
        require(owner->degree == degree, "Canonical creation changed degree");
        const bool restored = vb_preserve_curve_basis(*raw, expected_knots);
        if (restored) {
            std::vector<double> normalized_knots;
            const long double width = static_cast<long double>(end) - start;
            bool representable = true;
            for (const double k : expected_knots) {
                const double mapped = static_cast<double>((static_cast<long double>(k) - start) / width);
                require(std::isfinite(mapped), "Invalid normalized canonical knot");
                if (!normalized_knots.empty() && k > expected_knots[normalized_knots.size() - 1])
                    representable = representable && mapped > normalized_knots.back();
                normalized_knots.push_back(mapped);
            }
            if (representable) {
                SmBSplineCurve *query = nullptr;
                const auto copied = raw->Copy(*SmApiGetOrCreateContext(), query);
                owner->evaluation.reset(query);
                check(copied, "copy native query curve");
                require(query, "Missing native query copy");
                vb_preserve_curve_basis(*query, normalized_knots);
            }
        }
        SmTArray<SmPoint3d> exported_points;
        SmTArray<double> exported_weights, exported_knots;
        check(raw->GetControlPolygon(exported_points, exported_weights), "control polygon");
        check(raw->GetKnotsAll(exported_knots), "full knots");
        const bool polynomial = exported_weights.GetSize() == 0 && !raw->IsRational();
        const auto domain = raw->GetNaturalInterval();
        require(domain.GetMin() == start && domain.GetMax() == end,
                "Canonical creation changed the active NURBS interval");
        require(exported_points.GetSize() && exported_points.GetSize() <= limit &&
                    (polynomial || exported_points.GetSize() == exported_weights.GetSize()) &&
                    exported_knots.GetSize() == expected_knots.size(),
                "Invalid restored canonical data");
        for (ULONG i = 0; i < exported_points.GetSize(); ++i) {
            const auto &p = exported_points[i];
            const double w = polynomial ? 1.0 : exported_weights[i];
            require(std::isfinite(p.x) && std::isfinite(p.y) && std::isfinite(p.z) &&
                        std::isfinite(w) && w > 0, "Invalid canonical controls");
            owner->points.insert(owner->points.end(),
                                 {p.x, p.y, p.z, w});
        }
        owner->knots.assign(exported_knots.GetDataArray(),
                            exported_knots.GetDataArray() + exported_knots.GetSize());
        require(owner->knots == expected_knots, "Canonical creation changed the active knot data");
        *out = owner.release();
    });
}
extern "C" int vb_curve_evaluate(const VbCurve *curve, double parameter, double point[3],
                                 char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(curve && point && std::isfinite(parameter) &&
                    parameter >= curve->knots[curve->degree] &&
                    parameter <= curve->knots[curve->knots.size() - curve->degree - 1],
                "Parameter outside NURBS domain");
        SmVector3d result;
        SmVector3d *result_pointer = &result;
        SmVector3d *derivative = nullptr;
        SmVector3d *second_derivative = nullptr;
        auto *evaluator = curve->evaluation ? curve->evaluation.get() : curve->value.get();
        const double evaluated_parameter = curve->evaluation
            ? static_cast<double>((static_cast<long double>(parameter) - curve->knots[curve->degree]) /
                (static_cast<long double>(curve->knots[curve->knots.size() - curve->degree - 1]) -
                 curve->knots[curve->degree])) : parameter;
        check(SmApiEvaluateCurve(evaluator, evaluated_parameter, result_pointer, derivative,
                                 second_derivative),
              "evaluate curve");
        require(std::isfinite(result.x) && std::isfinite(result.y) && std::isfinite(result.z),
                "Nonfinite curve result");
        point[0] = result.x;
        point[1] = result.y;
        point[2] = result.z;
    });
}
extern "C" int vb_curve_sizes(const VbCurve *curve, size_t *degree, size_t *points, size_t *knots) {
    if (!curve || !degree || !points || !knots)
        return 1;
    *degree = curve->degree;
    *points = curve->points.size() / 4;
    *knots = curve->knots.size();
    return 0;
}
extern "C" int vb_curve_copy(const VbCurve *curve, double *points_xyzw, size_t point_count,
                             double *knots, size_t knot_count) {
    if (!curve || !points_xyzw || !knots || point_count != curve->points.size() / 4 ||
        knot_count != curve->knots.size())
        return 1;
    std::copy(curve->points.begin(), curve->points.end(), points_xyzw);
    std::copy(curve->knots.begin(), curve->knots.end(), knots);
    return 0;
}
extern "C" void vb_curve_free(VbCurve *curve) {
    std::lock_guard<std::mutex> lock(kernel_mutex);
    delete curve;
}

static std::unique_ptr<VbCurveIntersections> copy_curve_intersections(
    SmSolutionArray &solutions, const SmExtent1d &a, const SmExtent1d &b) {
        require(solutions.GetSize()<=500000,"Curve intersection exceeds event budget");
        auto result=std::make_unique<VbCurveIntersections>();
        for(ULONG i=0;i<solutions.GetSize();++i) {
            const auto &s=solutions[i];
            require(s.m_eSolutionType==SM_ST_SINGLE_VALUE || s.m_eSolutionType==SM_ST_RANGE_OF_VALUES,
                "Unsupported native curve intersection solution");
            const bool overlap=s.m_eSolutionType==SM_ST_RANGE_OF_VALUES;
            const double ta=s.m_vStart[0],tb=s.m_vStart[1];
            const double ea=overlap?s.m_vEnd[0]:ta,eb=overlap?s.m_vEnd[1]:tb;
            require(std::isfinite(ta) && std::isfinite(tb) && std::isfinite(ea) && std::isfinite(eb)
                && ta>=a.GetMin() && ta<=a.GetMax() && ea>=a.GetMin() && ea<=a.GetMax()
                && tb>=b.GetMin() && tb<=b.GetMax() && eb>=b.GetMin() && eb<=b.GetMax(),
                "Invalid native curve intersection parameters");
            result->events.push_back({overlap?1:0,{ta,ea},{tb,eb}});
        }
        return result;
}
extern "C" int vb_curve_intersect(const VbCurve *first,const VbCurve *second,double tolerance,
                                  VbCurveIntersections **out,char *error,size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(first && second && out && std::isfinite(tolerance) && tolerance>0,
            "Invalid curve intersection input");
        SmSolutionArray solutions;
        const auto a=first->value->GetNaturalInterval(),b=second->value->GetNaturalInterval();
        check(first->value->GlobalCurveIntersect(a,*second->value,b,tolerance,solutions),"intersect native curves");
        *out=copy_curve_intersections(solutions,a,b).release();
    });
}
extern "C" int vb_curve_self_intersect(const VbCurve *curve,double tolerance,
                                       VbCurveIntersections **out,char *error,size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(curve && out && std::isfinite(tolerance) && tolerance>0,
            "Invalid curve self-intersection input");
        SmSolutionArray solutions;
        const auto interval=curve->value->GetNaturalInterval();
        check(curve->value->GlobalCurveSelfIntersect(interval,tolerance,solutions),"intersect native curve with itself");
        *out=copy_curve_intersections(solutions,interval,interval).release();
    });
}
extern "C" size_t vb_curve_intersections_count(const VbCurveIntersections *events) {
    std::lock_guard<std::mutex> lock(kernel_mutex);
    return events?events->events.size():0;
}
extern "C" int vb_curve_intersections_copy(const VbCurveIntersections *events,VbCurveIntersection *out,size_t count) {
    std::lock_guard<std::mutex> lock(kernel_mutex);
    if(!events || count!=events->events.size() || (count && !out))return 1;
    if(count)std::copy(events->events.begin(),events->events.end(),out);
    return 0;
}
extern "C" void vb_curve_intersections_free(VbCurveIntersections *events) {
    std::lock_guard<std::mutex> lock(kernel_mutex);delete events;
}

extern "C" int vb_curve_surface_intersect(const VbCurve *curve,const VbNurbsData *surface,
    double tolerance,VbCurveSurfaceIntersections **out,char *error,size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(curve && surface && out && std::isfinite(tolerance) && tolerance>0,
                "Invalid curve/surface intersection input");
        require(surface->degree[0]<=17 && surface->degree[1]<=17,
                "Curve/surface intersection degree limit");
        std::unique_ptr<SmSurface> imported(vb_import_query_surface(*surface));
        const auto interval=curve->value->GetNaturalInterval();
        const auto domain=imported->GetNaturalUVDomain();
        SmSolutionArray solutions;
        check(imported->GlobalCurveIntersect(domain,*curve->value,interval,tolerance,solutions),
              "intersect native curve and surface");
        require(solutions.GetSize()<=500000,"Curve/surface intersection event budget");
        auto result=std::make_unique<VbCurveSurfaceIntersections>();
        for(ULONG i=0;i<solutions.GetSize();++i) {
            const auto &s=solutions[i];
            require(s.m_lNumVariables==3 && (s.m_eSolutionType==SM_ST_SINGLE_VALUE ||
                s.m_eSolutionType==SM_ST_RANGE_OF_VALUES),"Unsupported curve/surface solution");
            VbCurveSurfaceIntersection event{};
            event.overlap=s.m_eSolutionType==SM_ST_RANGE_OF_VALUES?1:0;
            for(int end=0;end<2;++end) {
                const auto &value=end && event.overlap?s.m_vEnd:s.m_vStart;
                const double t=value[0],u=value[1],v=value[2];
                require(std::isfinite(t) && std::isfinite(u) && std::isfinite(v)
                    && t>=interval.GetMin() && t<=interval.GetMax()
                    && u>=domain.GetMin().x && u<=domain.GetMax().x
                    && v>=domain.GetMin().y && v<=domain.GetMax().y,
                    "Invalid curve/surface intersection parameters");
                event.curve[end]=t;event.u[end]=u;event.v[end]=v;
            }
            result->events.push_back(event);
        }
        *out=result.release();
    });
}
extern "C" size_t vb_curve_surface_intersections_count(const VbCurveSurfaceIntersections *events) {
    std::lock_guard<std::mutex> lock(kernel_mutex);return events?events->events.size():0;
}
extern "C" int vb_curve_surface_intersections_copy(const VbCurveSurfaceIntersections *events,
    VbCurveSurfaceIntersection *out,size_t count) {
    std::lock_guard<std::mutex> lock(kernel_mutex);
    if(!events || count!=events->events.size() || (count && !out))return 1;
    if(count)std::copy(events->events.begin(),events->events.end(),out);
    return 0;
}
extern "C" void vb_curve_surface_intersections_free(VbCurveSurfaceIntersections *events) {
    std::lock_guard<std::mutex> lock(kernel_mutex);delete events;
}

namespace {
struct OwnedIntersectionCurves {
    SmTArray<SmCurve*> values;
    ~OwnedIntersectionCurves() { for(ULONG i=0;i<values.GetSize();++i)delete values[i]; }
};
VbSurfaceData *intersection_curve_snapshot(SmCurve &curve,double tolerance) {
    if(curve.IsKindOf(SmBSplineCurve_TYPE))
        return vb_export_curve(static_cast<SmBSplineCurve&>(curve));
    SmBSplineCurve *exact=nullptr;
    const auto exact_status=curve.MakeExactBSplineIfPossible(exact);
    std::unique_ptr<SmBSplineCurve> owner(exact);
    check(exact_status,"convert exact intersection curve");
    if(!exact) {
        SmTArray<double> breaks;
        const auto interval=curve.GetNaturalInterval();
        breaks.Add(interval.GetMin());breaks.Add(interval.GetMax());
        double deviation=0;
        const auto status=curve.ApproximateCurve(*SmApiGetOrCreateContext(),breaks,tolerance,deviation,exact,TRUE,TRUE,TRUE);
        owner.reset(exact);
        check(status,"approximate parameter-matched intersection curve");
        require(std::isfinite(deviation) && deviation<=tolerance,"Intersection curve approximation gap");
    }
    require(exact,"Missing intersection spline");
    return vb_export_curve(*exact);
}
}
extern "C" int vb_shape_silhouettes(const VbSolid *shape, const double toward_eye[3],
    double approximation, double angle, int internal_edges, VbCurveList **out, char *error, size_t capacity) {
    if(out) *out=nullptr;
    return guarded(error,capacity,[&] {
        require(shape && shape->value && out && finite3(toward_eye)
            && std::isfinite(approximation) && approximation>0 && approximation*0.5>0
            && std::isfinite(angle) && angle>0 && angle<=SM_PI
            && (internal_edges==0 || internal_edges==1),"Invalid silhouette input");
        SmVector3d direction(toward_eye[0],toward_eye[1],toward_eye[2]);
        check(direction.Unitize(),"normalize silhouette view direction");
        const auto &context=*SmApiGetOrCreateContext();
        // HCR owns its transformed scene and copies its visible intervals.
        // The original native shape is borrowed and never edited.
        SmHCR hcr(context,shape->value.get(),direction,approximation*0.5,angle,
                  internal_edges!=0,internal_edges!=0);
        hcr.SetHCRForSilhouettes(internal_edges==0);
        check(hcr.ComputeGlobalVisibility(),"compute parallel silhouette visibility");
        SmTArray<SmCurve*> curves;
        hcr.GetVisibleCurves(curves); // borrowed from hcr
        require(curves.GetSize()<=limit,"Silhouette curve count limit");
        auto result=std::make_unique<VbCurveList>();
        result->curves.reserve(curves.GetSize());
        for(ULONG i=0;i<curves.GetSize();++i) {
            require(curves[i],"Missing silhouette curve");
            result->curves.emplace_back(intersection_curve_snapshot(*curves[i],approximation*0.5));
        }
        *out=result.release();
    });
}
extern "C" size_t vb_curve_list_count(const VbCurveList *curves) {
    std::lock_guard<std::mutex> lock(kernel_mutex);
    return curves?curves->curves.size():0;
}
extern "C" int vb_curve_list_take(VbCurveList *curves,size_t index,VbSurfaceData **out) {
    if(out)*out=nullptr;
    std::lock_guard<std::mutex> lock(kernel_mutex);
    if(!curves || !out || index>=curves->curves.size() || !curves->curves[index])return 1;
    *out=curves->curves[index].release();return 0;
}
extern "C" void vb_curve_list_free(VbCurveList *curves) {
    std::lock_guard<std::mutex> lock(kernel_mutex);delete curves;
}

extern "C" int vb_surface_intersect(const VbNurbsData *first,const VbNurbsData *second,
    double tolerance,double angle,VbSurfaceIntersections **out,char *error,size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(first && second && out && std::isfinite(tolerance) && tolerance>0
            && std::isfinite(angle) && angle>0,"Invalid surface intersection input");
        require(first->degree[0]<=17 && first->degree[1]<=17 && second->degree[0]<=17 && second->degree[1]<=17,
            "Surface intersection degree limit");
        std::unique_ptr<SmSurface> a(vb_import_query_surface(*first)),b(vb_import_query_surface(*second));
        OwnedIntersectionCurves xyz,uv1,uv2;
        SmTArray<SmTsectCurveType> types;SmTArray<double> deviations;
        const SmBoolean edges[]={TRUE,TRUE};const SmApproxTol3d approximation(tolerance*0.25);
        check(a->GlobalSurfaceIntersect(*SmApiGetOrCreateContext(),a->GetNaturalUVDomain(),*b,b->GetNaturalUVDomain(),
            edges,&approximation,&angle,&xyz.values,&uv1.values,&uv2.values,&types,&deviations),"intersect native surfaces");
        const ULONG count=xyz.values.GetSize();
        require(count<=500000 && uv1.values.GetSize()==count && uv2.values.GetSize()==count
            && types.GetSize()==count && deviations.GetSize()==count,"Inconsistent surface intersection arrays");
        auto result=std::make_unique<VbSurfaceIntersections>();
        for(ULONG i=0;i<count;++i) {
            require(xyz.values[i],"Missing spatial intersection curve");
            SurfaceIntersectionEntry entry{};
            entry.kind=static_cast<int>(types[i]);
            require(entry.kind>=SM_TC_TOUCHING && entry.kind<=SM_TC_REGION_BOUNDARY,"Unknown surface intersection classification");
            require(std::isfinite(deviations[i]) && deviations[i]>=0,"Invalid surface intersection deviation");
            if(types[i]==SM_TC_TOUCHING) {
                SmPoint3d point;
                check(xyz.values[i]->EvaluatePoint(xyz.values[i]->GetNaturalInterval().GetMin(),point),"evaluate surface contact point");
                entry.point[0]=point.x;entry.point[1]=point.y;entry.point[2]=point.z;
                require(finite3(entry.point),"Nonfinite surface contact point");
            } else {
                require(uv1.values[i] && uv2.values[i],"Missing surface intersection UV curve");
                entry.curves[0].reset(intersection_curve_snapshot(*xyz.values[i],tolerance*0.25));
                entry.curves[1].reset(intersection_curve_snapshot(*uv1.values[i],tolerance*0.25));
                entry.curves[2].reset(intersection_curve_snapshot(*uv2.values[i],tolerance*0.25));
            }
            result->entries.push_back(std::move(entry));
        }
        *out=result.release();
    });
}
// The high-level API fixes its fitting tolerance. Use the public surface
// operation so both the normal-drop trace and lifted image honor the caller.
extern "C" int vb_surface_pull(const VbCurve *curve,const VbNurbsData *surface,
    double tolerance,VbSurfaceIntersections **out,char *error,size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(curve && surface && out && std::isfinite(tolerance) && tolerance*0.125>0,
                "Invalid surface pull input");
        require(surface->degree[0]<=16 && surface->degree[1]<=16,"Surface pull degree limit");
        std::unique_ptr<SmSurface> target(vb_import_query_surface(*surface));
        struct OwnedSplines {
            SmTArray<SmBSplineCurve*> values;
            ~OwnedSplines() { for(ULONG i=0;i<values.GetSize();++i)delete values[i]; }
        } uv;
        const auto &context=*SmApiGetOrCreateContext();
        double drop=0,gap=0;
        const double approximation=tolerance*0.125;
        check(target->DropAndTrimCurve(context,target->GetNaturalUVDomain(),
            *curve->value,curve->value->GetNaturalInterval(),approximation,
            drop,gap,uv.values,TRUE,FALSE),"pull curve along surface normals");
        require(uv.values.GetSize()<=500000 && std::isfinite(drop) && drop>=0 &&
                std::isfinite(gap) && gap>=0 && gap<=approximation,
                "Invalid surface pull trace or normal deviation");
        auto result=std::make_unique<VbSurfaceIntersections>();
        for(ULONG i=0;i<uv.values.GetSize();++i) {
            require(uv.values[i],"Missing surface pull UV curve");
            SmBSplineCurve *lifted=nullptr;double deviation=0;
            const auto status=target->LiftCurve(context,target->GetNaturalUVDomain(),
                *uv.values[i],uv.values[i]->GetNaturalInterval(),approximation,deviation,lifted);
            std::unique_ptr<SmBSplineCurve> owner(lifted);
            check(status,"lift surface pull curve");
            require(lifted && std::isfinite(deviation) && deviation>=0 && deviation<=approximation,
                    "Invalid surface pull lift or deviation");
            SurfaceIntersectionEntry entry{};
            entry.curves[0].reset(vb_export_curve(*lifted));
            entry.curves[1].reset(vb_export_curve(*uv.values[i]));
            result->entries.push_back(std::move(entry));
        }
        *out=result.release();
    });
}
extern "C" size_t vb_surface_intersections_count(const VbSurfaceIntersections *events) {
    std::lock_guard<std::mutex> lock(kernel_mutex);return events?events->entries.size():0;
}
extern "C" int vb_surface_intersections_info(const VbSurfaceIntersections *events,size_t index,int *kind,double point[3]) {
    std::lock_guard<std::mutex> lock(kernel_mutex);
    if(!events || index>=events->entries.size() || !kind || !point)return 1;
    const auto &entry=events->entries[index];*kind=entry.kind;std::copy(entry.point,entry.point+3,point);return 0;
}
extern "C" int vb_surface_intersections_take_curve(VbSurfaceIntersections *events,size_t index,size_t part,VbSurfaceData **out) {
    if(out)*out=nullptr;
    std::lock_guard<std::mutex> lock(kernel_mutex);
    if(!events || index>=events->entries.size() || part>=3 || !out || !events->entries[index].curves[part])return 1;
    *out=events->entries[index].curves[part].release();return 0;
}
extern "C" void vb_surface_intersections_free(VbSurfaceIntersections *events) {
    std::lock_guard<std::mutex> lock(kernel_mutex);delete events;
}

extern "C" int vb_curve_offset_approx(const VbCurve *source,const double normal[3],
                                      double distance,double tolerance,VbCurve **out,
                                      char *error,size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(source && out && finite3(normal) && std::isfinite(distance)
            && std::isfinite(tolerance) && tolerance>0,"Invalid offset approximation input");
        SmVector3d n(normal[0],normal[1],normal[2]);check(n.Unitize(),"normalize offset plane");
        const auto domain=source->value->GetNaturalInterval();
        SmTArray<double> breaks;breaks.Add(domain.GetMin());
        for(double k:source->knots) {
            if(k>domain.GetMin() && k<domain.GetMax() && k!=breaks.GetLast())breaks.Add(k);
        }
        breaks.Add(domain.GetMax());
        // SmOffsetCurve uses tangent x normal (right); Rust uses normal x tangent (left).
        class BoundedOffset : public SmOffsetCurve {
            mutable size_t evaluations=0;
            void consume() const {
                require(++evaluations<=128*8192,"Offset approximation exceeds evaluation budget");
            }
        public:
            BoundedOffset(const SmCurve &curve,const SmExtent1d &interval,const SmVector3d &normal,double d)
                : SmOffsetCurve(3,curve,interval,normal,d) {}
            SmStatus Evaluate(double t,ULONG count,SmBoolean left,SmVector3d derivatives[],
                              SmBoolean nonzero=TRUE) const override {
                consume();return SmOffsetCurve::Evaluate(t,count,left,derivatives,nonzero);
            }
            SmStatus EvaluatePoint(double t,SmPoint3d &point) const override {
                consume();return SmOffsetCurve::EvaluatePoint(t,point);
            }
        } offset(*source->value,domain,n,-distance);
        SmBSplineCurve *raw=nullptr;double gap=0;
        const auto status=offset.ApproximateCurve(*SmApiGetOrCreateContext(),breaks,tolerance,gap,raw,FALSE,TRUE,FALSE);
        std::unique_ptr<SmBSplineCurve> result(raw);check(status,"approximate pointwise offset");
        require(result && std::isfinite(gap) && gap<=tolerance,"Invalid native offset approximation");
        auto owner=std::make_unique<VbCurve>();owner->degree=result->GetDegree();
        SmTArray<SmPoint3d> points;SmTArray<double> weights,knots;
        check(result->GetControlPolygon(points,weights),"export offset controls");
        require(points.GetSize()<=24577 && (weights.GetSize()==0 || weights.GetSize()==points.GetSize()),"Offset approximation exceeds control budget");
        for(ULONG i=0;i<points.GetSize();++i) {
            const auto p=points[i];owner->points.insert(owner->points.end(),{p.x,p.y,p.z,weights.GetSize()?weights[i]:1.});
        }
        check(result->GetKnotsAll(knots),"export offset knots");
        owner->knots.assign(knots.GetDataArray(),knots.GetDataArray()+knots.GetSize());
        owner->value=std::move(result);*out=owner.release();
    });
}

extern "C" int vb_curve_fillet_contacts(const VbCurve *first, const VbCurve *second,
                                        double radius, double tolerance, double parameters[2],
                                        double arc_samples[9], char *error, size_t capacity) {
    return guarded(error,capacity,[&] {
        require(first && second && parameters && arc_samples && std::isfinite(radius) && radius>0
            && std::isfinite(tolerance) && tolerance>0,"Invalid fillet contact input");
#ifdef VIBOCEROS_CURVE_FILLET_SEED_API
        class Corner : public SmCurveFilletExecutive {
        public:
            Corner(SmBSplineCurve *a,SmBSplineCurve *b,double r)
                : SmCurveFilletExecutive(a,b,r,FALSE,FALSE) {
                m_dParam1=a->GetNaturalInterval().GetMin();
                m_dParam2=b->GetNaturalInterval().GetMax();
            }
        };
        Corner worker(first->value.get(),second->value.get(),radius);
        SmBSplineCurve *raw=nullptr;
        const auto a=first->value->GetNaturalInterval(),b=second->value->GetNaturalInterval();
        const auto status=worker.DoFillet(raw,a.GetMax(),b.GetMin(),tolerance,parameters);
        std::unique_ptr<SmBSplineCurve> result(raw?raw:worker.GetResult());
        check(status,"solve native fillet contacts");
        require(result!=nullptr,"Missing native fillet arc");
        const auto domain=result->GetNaturalInterval();
        for(size_t i=0;i<3;++i) {
            SmPoint3d point;
            check(result->EvaluatePoint(domain.Evaluate(i*0.5),point),"evaluate native fillet arc");
            arc_samples[3*i]=point.x;arc_samples[3*i+1]=point.y;arc_samples[3*i+2]=point.z;
        }
#else
        throw std::runtime_error("Seeded curve filleting is unavailable in this trial kernel build");
#endif
    });
}

extern "C" int vb_curve_extrude(const VbCurve *curve, const double displacement[3],
                                VbSurfaceData **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(curve && out && finite3(displacement), "Invalid extrusion arguments");
        const SmVector3d vector(displacement[0], displacement[1], displacement[2]);
        require(std::isfinite(vector.Length()) && vector.Length() > 0,
                "Invalid extrusion displacement");
        SmSurface *raw = nullptr;
        const auto status = SmApiCreateExtrude(curve->value.get(), vector, raw);
        std::unique_ptr<SmSurface> owner(raw);
        check(status, "extrude curve surface");
        require(raw && raw->IsKindOf(SmBSplineSurface_TYPE), "Missing NURBS extrusion surface");
        auto *surface = static_cast<SmBSplineSurface *>(raw);
        vb_preserve_surface_basis(*surface, {std::vector<double>{0, 0, 1, 1}, curve->knots});
        // Native extrusion is linear in U. The application uses profile U and
        // physical-distance V, so use the native chart operations to align it.
        auto swapped = vb_swapped_surface_uv(*surface);
        surface = swapped.get();
        owner.reset(swapped.release());
        check(surface->Reparameterize(SmExtent2d(curve->knots.front(), 0,
                                                curve->knots.back(), vector.Length())),
              "set extrusion distance domain");
        vb_preserve_surface_basis(*surface, {curve->knots, {0, 0, vector.Length(), vector.Length()}});
        *out = vb_export_surface(*surface);
    });
}

extern "C" int vb_curves_plane_normal(const VbCurve *const *curves, size_t count, double tolerance,
                                       int *has_plane, double normal[3], char *error, size_t capacity) {
    if (has_plane) *has_plane = 0;
    return guarded(error, capacity, [&] {
        require(curves && count && count <= limit && has_plane && normal &&
                    std::isfinite(tolerance) && tolerance > 0, "Invalid curve group plane query");
        SmTArray<SmPoint3d> controls;
        for (size_t i = 0; i < count; ++i) {
            require(curves[i], "Missing curve group plane input");
            const auto &points = curves[i]->points;
            require(controls.GetSize() <= limit - points.size() / 4, "Curve group plane resource limit");
            const SmPoint3d origin(points[0], points[1], points[2]);
            for (size_t j = 0; j < points.size(); j += 4) {
                const auto relative = SmPoint3d(points[j], points[j + 1], points[j + 2]) - origin;
                require(std::isfinite(relative.x) && std::isfinite(relative.y) && std::isfinite(relative.z),
                        "Curve group plane coordinates exceed finite range");
                controls.Add(relative);
            }
        }
        SmPointSet3d set(controls, tolerance);
        if (set.IsLinear()) {
            controls.ReSet();
            for (size_t i = 0; i < count; ++i) {
                const auto &points = curves[i]->points;
                for (size_t j = 0; j < points.size(); j += 4)
                    controls.Add(SmPoint3d(points[j], points[j + 1], points[j + 2]));
            }
            SmPointSet3d positions(controls, tolerance);
            set = positions;
        }
        if (!set.IsPlanar()) return;
        auto direction = set.GetNormal();
        // GetNormal runs a different classifier through SetUpAxes and can
        // replace the cached point-set type. Accept only its planar result.
        if (!set.IsPlanar()) return;
        check(direction.Unitize(), "normalize modeling curve group plane");
        for (int i = 0; i < 3; ++i) {
            require(std::isfinite(direction[i]), "Invalid curve group plane normal");
            normal[i] = direction[i];
        }
        *has_plane = 1;
    });
}

extern "C" int vb_curve_apex_surface(const VbCurve *curve, const double apex[3],
                                      VbSurfaceData **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(curve && out, "Invalid apex surface arguments");
        auto surface = apex_surface(*curve, apex);
        *out = vb_export_surface(*surface);
    });
}

extern "C" int vb_curve_translational_sweep(const VbCurve *profile, const VbCurve *path,
                                           VbSurfaceData **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(profile && path && out, "Invalid translational sweep arguments");
        auto surface = translational_sweep(*profile, *path);
        *out = vb_export_surface(*surface);
    });
}

extern "C" int vb_shape_translational_sweep(const VbCurve *profile, const VbCurve *path,
                                           double tolerance, VbSolid **out, char *error,
                                           size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(profile && path && out && std::isfinite(tolerance) && tolerance > 0,
                "Invalid capped translational sweep arguments");
        SmVector3d normal;
        require(profile->value->IsClosed(profile->value->GetNaturalInterval()) &&
                    profile->value->IsPlanar(tolerance, nullptr, &normal),
                "Capped translational sweep requires a closed planar profile");
        require(!path->value->IsClosed(path->value->GetNaturalInterval()),
                "Capped translational sweep requires an open path");
        SmPoint3d start, end;
        check(path->value->EvaluatePoint(path->value->GetNaturalInterval().GetMin(), start),
              "evaluate capped sweep path start");
        check(path->value->EvaluatePoint(path->value->GetNaturalInterval().GetMax(), end),
              "evaluate capped sweep path end");
        require(std::abs((end - start).Dot(normal)) > tolerance,
                "Capped translational sweep must leave the profile plane");
        *out = capped_surface(translational_sweep(*profile, *path), 3, tolerance).release();
    });
}

extern "C" int vb_shape_apex_solid(const VbCurve *curve, const double apex[3], double tolerance,
                                    VbSolid **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(curve && out && finite3(apex) && std::isfinite(tolerance) && tolerance > 0,
                "Invalid apex solid arguments");
        SmPoint3d plane_point;
        SmVector3d normal;
        require(curve->value->IsClosed(curve->value->GetNaturalInterval()) &&
                    curve->value->IsPlanar(tolerance, &plane_point, &normal),
                "Apex solid requires a closed planar profile");
        require(std::abs((SmPoint3d(apex[0], apex[1], apex[2]) - plane_point).Dot(normal)) > tolerance,
                "Apex solid must leave the profile plane");
        *out = capped_surface(apex_surface(*curve, apex), 1, tolerance).release();
    });
}

namespace {
bool surface_control_plane_normal(SmSurface &surface,double tolerance,SmVector3d &normal) {
    require(surface.IsKindOf(SmBSplineSurface_TYPE),"Surface plane query requires NURBS");
    auto &spline=static_cast<SmBSplineSurface&>(surface);
    const size_t nu=spline.GetNumberControlPoints(SM_SP_U),nv=spline.GetNumberControlPoints(SM_SP_V);
    require(nu && nv && nu<=limit/nv,"Surface plane query control limit");
    ULONG count_u=0,count_v=0;SmTArray<SmPoint3d> points;SmTArray<double> weights;
    check(spline.GetControlPointNet(count_u,count_v,points,weights),"read surface plane controls");
    require(points.GetSize()>0,"Missing surface plane controls");
    const SmPoint3d origin=points[0];
    for(ULONG i=0;i<points.GetSize();++i) {
        points[i]-=origin;
        require(std::isfinite(points[i].x) && std::isfinite(points[i].y) && std::isfinite(points[i].z),
                "Surface plane coordinates exceed finite range");
    }
    // The generic surface predicate tests a padded pseudobox, which can
    // decline exact planes. Classify the NURBS controls using the kernel's
    // point-set classifier, as for numeric curve extrusion direction.
    SmPointSet3d set(points,tolerance);
    if(!set.IsPlanar())return false;
    normal=set.GetNormal();
    if(!set.IsPlanar())return false;
    check(normal.Unitize(),"normalize surface control plane");return true;
}
}
extern "C" int vb_shape_single_face_plane_normal(const VbSolid *shape, double tolerance,
    int *has_plane, double normal[3], char *error, size_t capacity) {
    if(has_plane)*has_plane=0;
    return guarded(error,capacity,[&] {
        require(shape && shape->value && has_plane && normal && std::isfinite(tolerance) && tolerance>0,
                "Invalid surface plane query");
        SmTArray<SmFace*> faces;shape->value->GetFaces(faces);
        require(faces.GetSize()==1,"Surface plane query requires exactly one face");
        SmVector3d direction;
        if(!surface_control_plane_normal(*faces[0]->GetSurface(),tolerance,direction))return;
        for(size_t i=0;i<3;++i)normal[i]=direction[i];
        require(finite3(normal),"Invalid surface plane normal");*has_plane=1;
    });
}

extern "C" int vb_shape_extrude_faces(const VbSolid *shape, const double displacement[3],
    int cap_ends, double tolerance, VbSolid **out, char *error, size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(shape && shape->value && out && finite3(displacement)
            && (cap_ends==0 || cap_ends==1) && std::isfinite(tolerance) && tolerance>0,
            "Invalid surface extrusion input");
        require(!shape->value->IsManifoldSolid(),"Surface extrusion requires an open sheet");
        SmVector3d direction(displacement[0],displacement[1],displacement[2]);
        const double distance=direction.Length();
        require(std::isfinite(distance) && distance>0,"Invalid surface extrusion displacement");
        check(direction.Unitize(),"normalize surface extrusion direction");
        auto owner=std::make_unique<VbSolid>();
        owner->value.reset(vb_copy_brep(*shape->value));
        owner->value->SetTolerance(tolerance,FALSE);
        SmTArray<SmFace*> faces;owner->value->GetFaces(faces);
        require(faces.GetSize()>0 && faces.GetSize()<=100000,"Surface extrusion face limit");
        for(ULONG i=0;i<faces.GetSize();++i) {
            SmVector3d normal;
            if(surface_control_plane_normal(*faces[i]->GetSurface(),tolerance,normal)) {
                require(std::abs(direction.Dot(normal))*distance>tolerance,
                        "Surface extrusion must leave a planar source face");
            }
        }
        // The primitive convenience overload redirects through boundary curves
        // under USE_NEW_ANALYTICS, rebuilding planar caps. The topology sweep
        // keeps the actual source surfaces, including curved and trimmed caps.
        owner->value->m_bEditingEnabled=TRUE;
        SmTranslationalSweepGeometry geometry(distance*direction,tolerance);
        SmTopologySweep sweep(geometry);sweep.SetDoStitching(FALSE);
        SmTArray<SmEdge*> all_edges,edges;owner->value->GetEdges(all_edges);
        for(ULONG i=0;i<all_edges.GetSize();++i)
            if(all_edges[i]->IsLamina())edges.Add(all_edges[i]);
        require(edges.GetSize()>0,"Surface extrusion requires a boundary");
        // Sweep boundary edges higher, and copy caps to their translated
        // position. Shared interior edges stay in the cap topology; sweeping
        // them higher would introduce interior walls on connected sheets.
        check(sweep.DoSweep(owner->value.get(),owner->value->GetInfiniteRegion(),nullptr,&edges,
              nullptr,cap_ends?&faces:nullptr),"sweep surface boundaries and caps");
        if(!cap_ends) {
            for(ULONG i=0;i<faces.GetSize();++i)
                check(owner->value->DeleteFace(faces[i],TRUE,TRUE),"remove source cap from wall sweep");
        }
        check(owner->value->StitchAndOrient(),"stitch and orient surface extrusion");
        owner->value->m_bEditingEnabled=FALSE;
        faces.ReSet();owner->value->GetFaces(faces);
        require(faces.GetSize()>0,"Empty surface extrusion");
        require(!cap_ends || owner->value->IsManifoldSolid(),"Surface extrusion did not close");
        check(SmApiTurnToNurbs(owner->value.get()),"convert surface extrusion to NURBS");
        *out=owner.release();
    });
}

extern "C" int vb_shape_extrude_faces_to_point(const VbSolid *shape, const double apex[3],
    int cap_base, double tolerance, VbSolid **out, char *error, size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(shape && shape->value && out && finite3(apex) && (cap_base==0 || cap_base==1)
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
            check(profile->SubdivideAtDiscontinuity(*context,SM_CT_G1,segments),"split apex source at tangents");
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

extern "C" int vb_shape_extrude_faces_along_curve(const VbSolid *shape, const VbCurve *path,
    int cap_ends, double tolerance, VbSolid **out, char *error, size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(shape && shape->value && path && path->value && out
            && (cap_ends==0 || cap_ends==1) && std::isfinite(tolerance) && tolerance>0,
            "Invalid surface path extrusion input");
        require(!shape->value->IsManifoldSolid(),"Surface path extrusion requires an open sheet");
        require(!path->value->IsDegenerate(tolerance),"Degenerate surface extrusion path");
        require(!path->value->IsClosed(path->value->GetNaturalInterval()),"Closed surface extrusion paths are not supported");
        SmTArray<SmFace*> source_faces;shape->value->GetFaces(source_faces);
        require(source_faces.GetSize()>0 && source_faces.GetSize()<=100000,"Surface path extrusion face limit");
        SmTArray<SmEdge*> all_edges,edges;shape->value->GetEdges(all_edges);
        const size_t path_controls=path->points.size()/4;
        require(path_controls>0,"Missing surface extrusion path controls");
        size_t profile_controls=0,max_degree=1;
        for(ULONG i=0;i<all_edges.GetSize();++i)if(all_edges[i]->IsLamina()) {
            auto *curve=all_edges[i]->GetCurve();
            require(curve && curve->IsKindOf(SmBSplineCurve_TYPE),"Surface path extrusion requires NURBS edges");
            const auto *nurbs=static_cast<SmBSplineCurve*>(curve);
            const size_t count=nurbs->GetNumberControlPoints();
            require(count<limit && profile_controls<limit-count,"Surface path extrusion profile resource limit");
            profile_controls+=count+1; // Allow a gap segment when native chains are assembled.
            max_degree=std::max(max_degree,static_cast<size_t>(nurbs->GetDegree()));
            edges.Add(all_edges[i]);
        }
        require(edges.GetSize()>0,"Surface path extrusion requires a boundary");
        // The producer combines each chain into one NURBS before sweeping.
        // Budget all chains conservatively, including Bezier decomposition and
        // elevation to a common degree, before allocating tensor-product nets.
        require(profile_controls<=limit/path_controls/(max_degree+1),
                "Surface path extrusion control-net resource limit");
        auto owner=std::make_unique<VbSolid>();
        owner->value.reset(new (*SmApiGetOrCreateContext()) SmBrep());
        owner->value->SetTolerance(tolerance,FALSE);
        SmSweepOptions options;
        options.bTranslationalSweep=TRUE;options.bMoveToProfile=TRUE;
        // Build only walls here; the primitive cap builder fits planar regions.
        // Copy actual source faces below to retain curved caps and trim holes.
        options.bCapEndsArg=FALSE;
        SmTArray<SmFace*> start,side,end;
        auto working_path=copy_curve(*path);
        SmPrimitiveCreation producer(owner->value->GetInfiniteRegion(),0,TRUE);
        check(producer.CreateCurveSweepFromEdges(edges,working_path.get(),nullptr,nullptr,tolerance,
              &options,start,side,end),"create translational surface path walls");
        SmPoint3d a,b;
        const auto interval=path->value->GetNaturalInterval();
        check(path->value->EvaluatePoint(interval.GetMin(),a),"evaluate surface path start");
        check(path->value->EvaluatePoint(interval.GetMax(),b),"evaluate surface path end");
        if(cap_ends) {
            check(shape->value->CopyFaces(source_faces,owner->value.get(),nullptr,FALSE),"copy source surface caps");
            std::unique_ptr<SmBrep> translated(vb_copy_brep(*shape->value));
            SmAxis2Placement move;move.Translate(b-a);
            check(translated->Transform(move),"translate surface path caps");
            SmTArray<SmFace*> caps;translated->GetFaces(caps);
            check(translated->CopyFaces(caps,owner->value.get(),nullptr,FALSE),"copy translated surface caps");
        }
        check(owner->value->StitchAndOrient(),"stitch surface path extrusion");
        SmTArray<SmFace*> result_faces;owner->value->GetFaces(result_faces);
        require(result_faces.GetSize()>0 && result_faces.GetSize()<=100000,"Surface path extrusion output face limit");
        require(!cap_ends || owner->value->IsManifoldSolid(),"Surface path extrusion did not close");
        check(SmApiTurnToNurbs(owner->value.get()),"convert surface path extrusion to NURBS");
        *out=owner.release();
    });
}

extern "C" int vb_shape_tapered_face(const VbSolid *shape,const double direction[3],
    double height,double angle_degrees,int round_corners,int cap_ends,double tolerance,
    VbSolid **out,char *error,size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(shape && shape->value && out && finite3(direction) && std::isfinite(height) && height>0
            && std::isfinite(angle_degrees) && std::abs(angle_degrees)<90 && angle_degrees!=0
            && (round_corners==0 || round_corners==1) && (cap_ends==0 || cap_ends==1)
            && std::isfinite(tolerance) && tolerance*0.001>0,"Invalid tapered surface extrusion input");
        const double offset=height*std::tan(angle_degrees*SM_PI/180.0);
        require(std::isfinite(offset) && offset!=0,"Unrepresentable surface taper offset");
        auto *context=SmApiGetOrCreateContext();SmVector3d normal(direction[0],direction[1],direction[2]);
        check(normal.Unitize(),"normalize surface taper direction");
        SmTArray<SmFace*> source_faces;shape->value->GetFaces(source_faces);
        require(source_faces.GetSize()==1 && !shape->value->IsManifoldSolid(),"Surface taper requires one open face");
        auto *source=source_faces[0];SmTArray<SmLoop*> loops;source->GetLoops(loops);
        require(loops.GetSize()>0 && loops.GetSize()<=100000,"Surface taper loop resource limit");
        auto owner=std::make_unique<VbSolid>();owner->value.reset(new (*context) SmBrep());
        owner->value->SetTolerance(tolerance,FALSE);size_t controls=0,total_faces=0;
        for(ULONG i=0;i<loops.GetSize();++i) {
            SmTArray<SmEdge*> edges;loops[i]->GetEdges(edges);
            require(edges.GetSize()>0,"Surface taper does not support vertex loops");
            for(ULONG j=0;j<edges.GetSize();++j) {
                require(edges[j]->IsLamina(),"Surface taper does not support seam loops");
                auto *curve=edges[j]->GetCurve();
                require(curve && curve->IsKindOf(SmBSplineCurve_TYPE),"Surface taper requires NURBS boundaries");
                const size_t count=static_cast<SmBSplineCurve*>(curve)->GetNumberControlPoints();
                require(count<=limit/2 && controls<=limit/2-count,"Surface taper control-net resource limit");controls+=count;
            }
            SmOrientType orientation;
            check(source->ComputeLoopOrientation(loops[i],orientation),"classify taper hole loops");
            require(orientation==SM_OT_SAME || orientation==SM_OT_OPPOSITE,"Unclassified surface taper boundary");
            SmTArray<ULONG> sizes;SmTArray<SmBSplineCurve*> boundary;
            SmObjsDelete<SmBSplineCurve*> boundary_cleanup(&boundary);
            check(source->CreateOrientedTrimBoundary(*context,FALSE,FALSE,sizes,boundary,loops[i]),"copy taper trim boundary");
            require(boundary.GetSize()>0 && boundary.GetSize()<=100000,"Surface taper boundary resource limit");
            SmTArray<SmCurve*> profiles;for(ULONG j=0;j<boundary.GetSize();++j)profiles.Add(boundary[j]);
            boundary_cleanup.Clear();SmObjsDelete<SmCurve*> input_cleanup(&profiles);
            SmTArray<ULONG> chains;
            require(SmCurve::FixChainCurvesList(profiles,chains,2*tolerance,TRUE)==1,"Surface taper boundary must be one closed chain");
            std::unique_ptr<SmBrep> walls(new (*context) SmBrep());walls->SetTolerance(tolerance,FALSE);
            SmPrimitiveCreation producer(walls->GetInfiniteRegion(),0,TRUE);
            SmVector3d loop_normal=normal;
            check(producer.CreateTaperExtrude(profiles,height,orientation==SM_OT_OPPOSITE?-angle_degrees:angle_degrees,
                0,loop_normal,round_corners?SM_OC_FILLET_CORNER:SM_OC_LINEAR_EXTENSION),"create tapered surface boundary walls");
            input_cleanup.Clear(); // successful producer consumes its copied profiles
            SmTArray<SmFace*> faces;walls->GetFaces(faces);total_faces+=faces.GetSize();
            require(faces.GetSize()>0 && total_faces<=100000,"Surface taper wall resource limit");
            check(walls->CopyFaces(faces,owner->value.get(),nullptr,FALSE),"collect surface taper walls");
        }
        if(cap_ends) {
            check(shape->value->CopyFaces(source_faces,owner->value.get(),nullptr,FALSE),"retain original taper base cap");
            check(owner->value->StitchAndOrient(),"stitch surface taper base");
            // With the actual base attached, only the offset end stays naked.
            // The native planar-face builder classifies nested top loops together.
            SmTArray<SmEdge*> edges;owner->value->GetEdges(edges);
            SmTArray<SmCurve*> profiles;std::vector<std::unique_ptr<SmCurve>> copies;
            size_t cap_controls=0;
            for(ULONG i=0;i<edges.GetSize();++i)if(edges[i]->IsLamina()) {
                auto *original=edges[i]->GetCurve();
                require(original && original->IsKindOf(SmBSplineCurve_TYPE),"Taper top requires NURBS edges");
                auto *spline=static_cast<SmBSplineCurve*>(original);
                const size_t count=spline->GetNumberControlPoints();
                require(count<=limit && cap_controls<=limit-count,"Surface taper cap resource limit");cap_controls+=count;
                std::unique_ptr<SmBSplineCurve> copy(new (*context) SmBSplineCurve(*spline));
                vb_preserve_curve_basis(*copy,vb_curve_full_knots(*spline));
                auto interval=edges[i]->GetInterval();check(copy->Trim(interval,FALSE),"trim tapered top edge");
                profiles.Add(copy.get());copies.emplace_back(std::move(copy));
            }
            require(profiles.GetSize()>0,"Surface taper has no offset boundary");
            SmTArray<SmFace*> caps;
            const auto status=owner->value->CreatePlanarFacesWith3DCurves(owner->value->GetInfiniteRegion(),profiles,tolerance,caps);
            if(status!=SM_SUCCESS) {
                SmTArray<SmEdge*> attached;owner->value->GetEdges(attached);
                for(ULONG i=0;i<attached.GetSize();++i)for(ULONG j=0;j<profiles.GetSize();++j)
                    if(attached[i]->GetCurve()==profiles[j]){attached[i]->SetCurve(nullptr,FALSE,FALSE,FALSE);break;}
                check(status,"cap tapered surface offset boundary");
            }
            for(auto &copy:copies)copy.release();
            require(caps.GetSize()>0,"Surface taper could not cap its offset end");
        }
        check(owner->value->StitchAndOrient(),"stitch tapered surface extrusion");
        SmTArray<SmFace*> result_faces;owner->value->GetFaces(result_faces);
        require(result_faces.GetSize()>0 && result_faces.GetSize()<=100000,"Surface taper output resource limit");
        require(!cap_ends || owner->value->IsManifoldSolid(),"Tapered surface extrusion did not close");
        check(SmApiTurnToNurbs(owner->value.get()),"convert surface taper to NURBS");*out=owner.release();
    });
}

extern "C" int vb_shape_tapered_extrusion(const VbCurve *curve, const double direction[3],
    double height, double angle_degrees, int round_corners, int cap_ends, double tolerance,
    VbSolid **out, char *error, size_t capacity) {
    if(out)*out=nullptr;
    return guarded(error,capacity,[&] {
        require(curve && curve->value && out && finite3(direction)
            && std::isfinite(height) && height!=0
            && std::isfinite(angle_degrees) && std::abs(angle_degrees)<90 && angle_degrees!=0
            && (round_corners==0 || round_corners==1) && (cap_ends==0 || cap_ends==1)
            && std::isfinite(tolerance) && tolerance*0.001>0,"Invalid tapered extrusion input");
        const double offset=height*std::tan(angle_degrees*SM_PI/180.0);
        require(std::isfinite(offset) && offset!=0,"Unrepresentable tapered extrusion offset");
        SmVector3d normal(direction[0],direction[1],direction[2]);
        check(normal.Unitize(),"normalize tapered extrusion direction");
        SmPoint3d origin;SmVector3d plane_normal;
        require(curve->value->IsPlanar(tolerance,&origin,&plane_normal),
                "Tapered extrusion requires a planar profile");
        require(!cap_ends || curve->value->IsClosed(curve->value->GetNaturalInterval()),
                "Capped tapered extrusion requires a closed profile");
        require(!curve->value->IsDegenerate(tolerance),"Degenerate tapered extrusion profile");
        auto copy=copy_curve(*curve);
        auto *context=SmApiGetOrCreateContext();
        SmTArray<SmBSplineCurve*> segments;
        SmObjsDelete<SmBSplineCurve*> segment_cleanup(&segments);
        check(copy->SubdivideAtDiscontinuity(*context,SM_CT_G1,segments),"split taper profile");
        require(segments.GetSize()>0 && segments.GetSize()<=100000,"Taper profile segment limit");
        SmTArray<SmCurve*> profiles;
        for(ULONG i=0;i<segments.GetSize();++i)profiles.Add(segments[i]);
        segment_cleanup.Clear();
        SmObjsDelete<SmCurve*> input_cleanup(&profiles);
        SmTArray<ULONG> chains;
        require(SmCurve::FixChainCurvesList(profiles,chains,2*tolerance,TRUE)==1,
                "Taper profile must form one connected chain");
        auto owner=std::make_unique<VbSolid>();
        owner->value.reset(new (*context) SmBrep());
        owner->value->SetTolerance(tolerance,FALSE);
        SmPrimitiveCreation producer(owner->value->GetInfiniteRegion(),0,TRUE);
        // On failure before caps the upstream producer retains these profiles.
        // Success consumes them; clear the guard before further validation.
        check(producer.CreateTaperExtrude(profiles,height,angle_degrees,cap_ends?3:0,normal,
            round_corners?SM_OC_FILLET_CORNER:SM_OC_LINEAR_EXTENSION),"create tapered extrusion");
        input_cleanup.Clear();
        SmTArray<SmFace*> faces;owner->value->GetFaces(faces);
        require(faces.GetSize()>0,"Empty tapered extrusion");
        require(!cap_ends || owner->value->IsManifoldSolid(),"Tapered extrusion did not close");
        check(SmApiTurnToNurbs(owner->value.get()),"convert tapered extrusion to NURBS");
        *out=owner.release();
    });
}

extern "C" int vb_shape_linear_sweep(const VbCurve *const *curves, size_t count,
                                      const double displacement[3], int cap_ends, double tolerance,
                                      VbSolid **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(curves && count && count <= 128 && out && finite3(displacement) &&
                    (cap_ends == 0 || cap_ends == 1) && std::isfinite(tolerance) && tolerance > 0,
                "Invalid linear sweep arguments");
        auto *context = SmApiGetOrCreateContext();
        SmTArray<SmCurve *> profiles;
        std::vector<std::unique_ptr<SmCurve>> copies;
        for (size_t i = 0; i < count; ++i) {
            require(curves[i] && curves[i]->value, "Missing linear sweep profile");
            auto copy = copy_curve(*curves[i]);
            profiles.Add(copy.get());
            copies.emplace_back(std::move(copy));
        }
        SmVector3d vector(displacement[0], displacement[1], displacement[2]);
        const double distance = vector.Length();
        require(std::isfinite(distance) && distance > 0, "Invalid linear sweep displacement");
        check(vector.Unitize(), "normalize linear sweep direction");
        if (cap_ends) {
            SmVector3d normal;
            require(profiles[0]->AreCoPlanar(profiles, tolerance, nullptr, &normal),
                    "Capped sweep requires coplanar profiles");
            require(std::abs(vector.Dot(normal)) * distance > tolerance,
                    "A capped sweep must cross the profile plane");
        }
        auto owner = std::make_unique<VbSolid>();
        owner->value.reset(new (*context) SmBrep());
        owner->value->SetTolerance(tolerance, FALSE);
        // SM_API has no creation-tolerance argument. Invoke the same upstream
        // producer on an explicitly configured B-rep. It consumes the private
        // profile copies, including its failure path; originals stay owned.
        SmPrimitiveCreation producer(owner->value->GetInfiniteRegion(), 0, TRUE);
        for (auto &copy : copies) copy.release();
        check(producer.CreateLinearSweep(profiles, vector, distance, 1, cap_ends),
              "create linear sweep");
        require(!cap_ends || owner->value->IsManifoldSolid(),
                "Capped sweep did not produce a manifold solid");
        check(SmApiTurnToNurbs(owner->value.get()), "convert exact linear sweep to NURBS");
        *out = owner.release();
    });
}

extern "C" int vb_curve_revolve(const VbCurve *curve, const double origin[3], const double axis[3],
                                 double angle_degrees, double parameter_length, VbSurfaceData **out,
                                 char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(curve && out && finite3(origin) && finite3(axis) &&
                    std::isfinite(angle_degrees) && angle_degrees > 0 && angle_degrees <= 360 &&
                    std::isfinite(parameter_length) && parameter_length > 0,
                "Invalid surface revolution arguments");
        SmVector3d direction(axis[0], axis[1], axis[2]);
        check(direction.Unitize(), "normalize revolution axis");
        SmBSplineSurface *raw = nullptr;
        auto working = canonical_working_curve(*curve);
        auto *profile = working ? working.get() : curve->value.get();
        // The high-level wrapper returns only the first surface after splitting
        // profile kinks. The existing tensor constructor retains the full curve.
        // Its bSwapUV header description is inverted in the pinned release:
        // FALSE constructs circular U and profile V.
        const auto status = SmBSplineSurface::CreateSurfOfRevolution(*SmApiGetOrCreateContext(),
            profile, SmPoint3d(origin[0], origin[1], origin[2]), direction,
            angle_degrees, raw, FALSE);
        std::unique_ptr<SmBSplineSurface> owner(raw);
        check(status, "create revolution surface");
        require(raw, "Missing revolution surface");
        auto knots = vb_surface_full_knots(*raw);
        knots[1] = vb_curve_full_knots(*profile);
        vb_preserve_surface_basis(*raw, knots);
        const auto domain = raw->GetNaturalUVDomain();
        check(raw->Reparameterize(SmExtent2d(0, domain.GetMin().y,
                                            parameter_length, domain.GetMax().y)),
              "set revolution parameter domain");
        knots = vb_surface_full_knots(*raw);
        // Circular U is generated by the native circle producer. Only restore
        // the supplied profile V basis, keeping those native circular controls.
        knots[1] = curve->knots;
        vb_preserve_surface_basis(*raw, knots);
        *out = vb_export_surface(*raw);
    });
}

extern "C" int vb_shape_planar_faces(const VbCurve *const *curves, size_t count,
                                      double tolerance, VbSolid **out, char *error, size_t capacity) {
    if (out) *out = nullptr;
    return guarded(error, capacity, [&] {
        require(curves && count && count <= 128 && out && std::isfinite(tolerance) && tolerance > 0,
                "Invalid planar-face arguments");
        auto *context = SmApiGetOrCreateContext();
        auto owner = std::make_unique<VbSolid>();
        owner->value.reset(new (*context) SmBrep());
        owner->value->SetTolerance(tolerance, FALSE);
        SmTArray<SmCurve *> profiles;
        std::vector<std::unique_ptr<SmCurve>> copies;
        for (size_t i = 0; i < count; ++i) {
            require(curves[i] && curves[i]->value, "Missing planar-face profile");
            auto copy = copy_curve(*curves[i]);
            profiles.Add(copy.get());
            copies.emplace_back(std::move(copy));
        }
        SmTArray<SmFace *> faces;
        const auto status = owner->value->CreatePlanarFacesWith3DCurves(
            owner->value->GetInfiniteRegion(), profiles, tolerance, faces);
        if (status != SM_SUCCESS) {
            // The same failure contract as SM_API: originals are caller-owned
            // on failure even when a partial B-rep already attached some curves.
            SmTArray<SmEdge *> edges;
            owner->value->GetEdges(edges);
            for (ULONG i = 0; i < edges.GetSize(); ++i)
                for (ULONG j = 0; j < profiles.GetSize(); ++j)
                    if (edges[i]->GetCurve() == profiles[j]) {
                        edges[i]->SetCurve(nullptr, FALSE, FALSE, FALSE);
                        break;
                    }
            check(status, "create planar faces");
        }
        for (auto &copy : copies) copy.release();
        require(faces.GetSize() > 0, "No planar faces were created");
        check(SmApiTurnToNurbs(owner->value.get()), "convert exact planar faces to NURBS");
        *out = owner.release();
    });
}

extern "C" int vb_curves_coplanar(const VbCurve *const *curves, size_t count, double tolerance,
                                    int *coplanar, char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(curves && count && count <= 128 && coplanar && std::isfinite(tolerance) && tolerance > 0,
                "Invalid coplanar curve query");
        SmTArray<SmCurve *> values;
        for (size_t i = 0; i < count; ++i) {
            require(curves[i] && curves[i]->value, "Missing coplanar curve");
            values.Add(curves[i]->value.get());
        }
        *coplanar = values[0]->AreCoPlanar(values, tolerance);
    });
}

extern "C" int vb_solid_empty(const VbSolid *solid, int *empty, char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(solid && empty, "Invalid empty query");
        SmTArray<SmFace *> faces;
        solid->value->GetFaces(faces);
        *empty = faces.GetSize() == 0;
    });
}

extern "C" int vb_solid_parts(const VbSolid *solid, VbParts **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(solid && out, "Invalid solid parts query");
        SmBrep *raw = nullptr;
        const auto copied = SmApiBrepCopy(solid->value.get(), raw);
        std::unique_ptr<SmBrep> copy(raw);
        check(copied, "copy for material parts");
        SmTArray<SmBrep *> bodies;
        SmBrep *separate = copy.release();
        struct Cleanup {
            SmBrep *&source;
            SmTArray<SmBrep *> &bodies;
            ~Cleanup() {
                bool listed = false;
                for (ULONG i = 0; i < bodies.GetSize(); ++i) {
                    if (bodies[i] == source)
                        listed = true;
                    delete bodies[i];
                }
                if (!listed)
                    delete source;
            }
        } cleanup{separate, bodies};
        check(SmBrep::CreateOneBrepPerBody(separate, bodies, FALSE), "separate material bodies");
        require(bodies.GetSize() <= 128, "Material part resource limit");
        auto parts = std::make_unique<VbParts>();
        parts->values.reserve(bodies.GetSize());
        for (ULONG i = 0; i < bodies.GetSize(); ++i) {
            auto item = std::make_unique<VbSolid>();
            item->value.reset(bodies[i]);
            if (separate == bodies[i])
                separate = nullptr;
            bodies[i] = nullptr;
            parts->values.push_back(std::move(item));
        }
        separate = nullptr;
        *out = parts.release();
    });
}
extern "C" size_t vb_parts_count(const VbParts *parts) { return parts ? parts->values.size() : 0; }
extern "C" VbSolid *vb_parts_take(VbParts *parts, size_t index) {
    return parts && index < parts->values.size() ? parts->values[index].release() : nullptr;
}
extern "C" void vb_parts_free(VbParts *parts) {
    std::lock_guard<std::mutex> lock(kernel_mutex);
    delete parts;
}

extern "C" int vb_solid_boundary_contact(const VbSolid *a, const VbSolid *b, double tolerance,
                                         int *contact, char *error, size_t capacity) {
    return guarded(error, capacity, [&] {
        require(a && b && contact && std::isfinite(tolerance) && tolerance > 0,
                "Invalid boundary contact query");
        *contact = 0;
        SmTArray<SmFace *> first, second;
        a->value->GetFaces(first);
        b->value->GetFaces(second);
        size_t work = 0;
        for (ULONG i = 0; i < first.GetSize(); ++i)
            for (ULONG j = 0; j < second.GetSize(); ++j) {
                require(++work <= 65536, "Boundary contact work limit");
                SmTArray<SmCurve *> curves;
                SmTArray<SmPoint3d> points;
                const auto status = SmApiIntersectFaces(first[i], second[j], curves, &points);
                struct Cleanup {
                    SmTArray<SmCurve *> &curves;
                    ~Cleanup() {
                        for (ULONG k = 0; k < curves.GetSize(); ++k)
                            delete curves[k];
                    }
                } cleanup{curves};
                check(status, "intersect boundaries for contact");
                if (curves.GetSize() > 0) {
                    *contact = 1;
                    return;
                }
            }
    });
}

extern "C" int vb_solid_copy(const VbSolid *solid, VbSolid **out, char *error, size_t capacity) {
    if (out)
        *out = nullptr;
    return guarded(error, capacity, [&] {
        require(solid && out, "Invalid solid copy");
        auto owner = std::make_unique<VbSolid>();
        SmBrep *raw = nullptr;
        const auto status = SmApiBrepCopy(solid->value.get(), raw);
        owner->value.reset(raw);
        check(status, "copy native solid");
        *out = owner.release();
    });
}
