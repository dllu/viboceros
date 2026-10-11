#include "brep_data.h"
#include <SmApiBrep.h>
#include <SmApiQueries.h>
#include <SmBSplineCurve.h>
#include <SmBSplineSurface.h>
#include <SmBrep.h>
#include <SmEdge.h>
#include <SmEdgeuse.h>
#include <SmFace.h>
#include <SmFaceuse.h>
#include <SmLoopuse.h>
#include <SmRegion.h>
#include <SmVertex.h>
#include <SmVertexuse.h>
#include <algorithm>
#include <cmath>
#include <limits>
#include <memory>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <vector>

namespace {
constexpr size_t limit = 1'000'000;
void require(bool condition, const char *message) {
    if (!condition)
        throw std::runtime_error(message);
}
void check(SmStatus status, const char *operation) {
    if (status != SM_SUCCESS)
        throw std::runtime_error(std::string(operation) + " status " + std::to_string(status));
}
struct NurbsPayload {
    VbNurbsData view{};
    std::vector<double> points, knots[2];
    void finish() {
        view.points_xyzw = points.data();
        for (int d = 0; d < 2; ++d) {
            view.knots[d] = knots[d].data();
            view.knot_count[d] = knots[d].size();
        }
    }
};
std::vector<double> copy_knots(const double *source, size_t count) {
    require(source && count && count <= limit + 65, "Invalid native knot count");
    std::vector<double> knots(source, source + count);
    for (size_t i = 0; i < knots.size(); ++i)
        require(std::isfinite(knots[i]) && (!i || knots[i] >= knots[i - 1]),
                "Invalid native knots");
    return knots;
}
std::unique_ptr<NurbsPayload> curve_payload(const SmBSplineCurve &curve) {
    auto data = std::make_unique<NurbsPayload>();
    SmTArray<SmPoint3d> points;
    SmTArray<double> weights, knots;
    check(curve.GetControlPolygon(points, weights), "export curve controls");
    require(points.GetSize() <= limit &&
                (weights.GetSize() == 0 || points.GetSize() == weights.GetSize()),
            "Invalid curve controls");
    for (ULONG i = 0; i < points.GetSize(); ++i) {
        const auto &p = points[i];
        data->points.insert(data->points.end(),
                            {p.x, p.y, p.z, weights.GetSize() ? weights[i] : 1.});
    }
    check(curve.GetKnotsAll(knots), "export full curve knots");
    data->knots[0] = copy_knots(knots.GetDataArray(), knots.GetSize());
    data->view.degree[0] = curve.GetDegree();
    data->view.count[0] = points.GetSize();
    data->view.count[1] = 1;
    data->finish();
    return data;
}
std::unique_ptr<NurbsPayload> surface_payload(const SmBSplineSurface &surface) {
    auto data = std::make_unique<NurbsPayload>();
    SmTArray<SmPoint3d> points;
    SmTArray<double> weights;
    ULONG nu, nv;
    check(surface.GetControlPointNet(nu, nv, points, weights), "export surface controls");
    require(nu && nv && nu <= limit / nv && points.GetSize() == nu * nv &&
                (weights.GetSize() == 0 || weights.GetSize() == points.GetSize()),
            "Invalid surface controls");
    // SMLib enumerates U outside V; Viboceros stores U as the fastest index.
    for (ULONG v = 0; v < nv; ++v)
        for (ULONG u = 0; u < nu; ++u) {
            const auto index = u * nv + v;
            const auto &p = points[index];
            data->points.insert(data->points.end(),
                                {p.x, p.y, p.z, weights.GetSize() ? weights[index] : 1.});
        }
    ULONG knot_counts[2] = {0, 0};
    double *knot_arrays[2] = {nullptr, nullptr};
    check(surface.GetKnotsPointers(knot_counts[0], knot_counts[1], knot_arrays[0], knot_arrays[1]),
          "export full surface knots");
    for (int d = 0; d < 2; ++d) {
        const auto direction = d == 0 ? SM_SP_U : SM_SP_V;
        data->knots[d] = copy_knots(knot_arrays[d], knot_counts[d]);
        data->view.degree[d] = surface.GetDegree(direction);
    }
    data->view.count[0] = nu;
    data->view.count[1] = nv;
    data->finish();
    return data;
}
std::unique_ptr<NurbsPayload> uv_payload(const SmBSplineCurve &curve,
                                         const SmSurface &surface, bool &snapped) {
    auto data = curve_payload(curve);
    const auto domain = surface.GetNaturalUVDomain();
    const auto lo = domain.GetMin(), hi = domain.GetMax();
    const double minimum[2] = {lo.x, lo.y}, maximum[2] = {hi.x, hi.y};
    for (size_t i = 0; i < data->points.size(); i += 4) {
        auto *p = data->points.data() + i;
        for (int d = 0; d < 2; ++d) {
            const double boundary = std::clamp(p[d], minimum[d], maximum[d]);
            const double epsilon = 32 * std::numeric_limits<double>::epsilon() *
                std::max({std::abs(minimum[d]), std::abs(maximum[d]), maximum[d] - minimum[d]});
            if (p[d] == boundary || std::abs(p[d] - boundary) > epsilon) continue;
            SmPoint2d point;
            check(surface.SnapToKnots(SmPoint2d(p[0], p[1]), epsilon, point),
                  "snap native UV boundary roundoff");
            const double value = d == 0 ? point.x : point.y;
            if (value == boundary) {
                p[d] = value;
                snapped = true;
            }
        }
    }
    return data;
}
} // namespace

struct VbSurfaceData {
    std::unique_ptr<NurbsPayload> geometry;
};
VbSurfaceData *vb_export_surface(const SmBSplineSurface &surface) {
    auto result = std::make_unique<VbSurfaceData>();
    result->geometry = surface_payload(surface);
    return result.release();
}
extern "C" int vb_surface_data_view(const VbSurfaceData *surface, VbNurbsData *view) {
    if (!surface || !view) return 1;
    *view = surface->geometry->view;
    return 0;
}
extern "C" void vb_surface_data_free(VbSurfaceData *surface) {
    delete surface;
}

struct VbBrep {
    std::vector<VbVertexData> vertices;
    std::vector<VbEdgeData> edges;
    std::vector<VbTrimData> trims;
    std::vector<VbLoopData> loops;
    std::vector<VbFaceData> faces;
    std::vector<std::unique_ptr<NurbsPayload>> geometry;
    VbNurbsData keep(std::unique_ptr<NurbsPayload> data) {
        const auto view = data->view;
        geometry.push_back(std::move(data));
        return view;
    }
};

SmBrep *vb_copy_brep(const SmBrep &source) {
    SmBrep *raw = nullptr;
    const auto status = SmApiBrepCopy(&source, raw);
    std::unique_ptr<SmBrep> copy(raw);
    check(status, "copy boundary representation");
    require(copy != nullptr, "Missing copied boundary representation");
    return copy.release();
}

VbBrep *vb_export_brep(const SmBrep &source, double approximation) {
    std::unique_ptr<SmBrep> copy(vb_copy_brep(source));
    if (approximation > 0) {
        require(std::isfinite(approximation), "Invalid export approximation tolerance");
        double achieved = 0;
        check(copy->ApproximateWithBSplines(approximation, achieved), "approximate native offset geometry");
        require(std::isfinite(achieved) && achieved <= approximation,
                "Native approximation exceeded requested tolerance");
    }
    SmTArray<SmFace *> faces;
    SmTArray<SmEdge *> edges;
    SmTArray<SmVertex *> vertices;
    copy->GetFaces(faces);
    copy->GetEdges(edges);
    copy->GetVertices(vertices);
    require(faces.GetSize() <= limit && edges.GetSize() <= limit && vertices.GetSize() <= limit,
            "B-rep resource limit");
    for (ULONG i = 0; i < faces.GetSize(); ++i)
        require(faces[i]->GetSurface()->IsKindOf(SmBSplineSurface_TYPE),
                "Surface conversion would require approximation");
    for (ULONG i = 0; i < edges.GetSize(); ++i)
        require(edges[i]->GetCurve()->IsKindOf(SmBSplineCurve_TYPE),
                "Edge conversion would require approximation");
    check(SmApiTurnToNurbs(copy.get()), "convert exact geometry to NURBS");
    auto result = std::make_unique<VbBrep>();
    std::unordered_map<const SmVertex *, size_t> vertex_index;
    std::unordered_map<const SmEdge *, size_t> edge_index;
    for (ULONG i = 0; i < vertices.GetSize(); ++i)
        vertex_index.emplace(vertices[i], i);
    for (ULONG i = 0; i < edges.GetSize(); ++i)
        edge_index.emplace(edges[i], i);
    for (ULONG i = 0; i < faces.GetSize(); ++i) {
        auto *face = faces[i];
        SmFaceuse *positive = nullptr;
        SmFaceuse *negative = nullptr;
        face->GetFaceuses(positive, negative);
        const bool sheet_reversed = positive->GetOrientation() != SM_OT_SAME;
        if (positive->GetOrientation() != SM_OT_SAME)
            std::swap(positive, negative);
        require(positive->GetRegion() && negative->GetRegion(), "Missing face region");
        const bool sheet = positive->GetRegion() == negative->GetRegion();
        require(sheet || positive->GetRegion()->IsVoid() != negative->GetRegion()->IsVoid(),
                "Export requires a material boundary or sheet");
        // Loops follow the positive surface side; the face flip follows the void side.
        VbFaceData record{
            result->keep(surface_payload(*static_cast<SmBSplineSurface *>(face->GetSurface()))),
            result->loops.size(), 0, sheet ? sheet_reversed : !positive->GetRegion()->IsVoid()};
        SmTArray<SmLoopuse *> loops;
        positive->GetLoopuses(loops);
        for (ULONG j = 0; j < loops.GetSize(); ++j) {
            auto *loop = loops[j];
            require(loop->IsEdgeLoopuse(), "Vertex-only loops require explicit seam topology");
            VbLoopData loop_record{result->trims.size(), 0,
                                   loop->GetOrientation() == SM_OT_OPPOSITE};
            SmTArray<SmEdgeuse *> uses;
            loop->GetEdgeuses(uses);
            for (ULONG k = 0; k < uses.GetSize(); ++k) {
                auto *use = uses[k];
                auto *edge = use->GetEdge();
                require(edge, "Singular edgeuse requires explicit pole topology");
                SmBSplineCurve *uv = nullptr;
                check(use->GetOrCreateUVTrimCurve(uv), "create UV trim after NURBS conversion");
                require(uv, "Missing UV trim curve");
                const bool reversed = use->GetOrientation() == SM_OT_OPPOSITE;
                bool snapped = false;
                auto payload = uv_payload(*uv, *face->GetSurface(), snapped);
                const auto a = vertex_index.at(edge->GetStartVertex()),
                           b = vertex_index.at(edge->GetEndVertex());
                result->trims.push_back({{reversed ? b : a, reversed ? a : b},
                                         edge_index.at(edge),
                                         reversed,
                                         snapped,
                                         result->keep(std::move(payload))});
                require(result->trims.size() <= limit, "Trim resource limit");
                ++loop_record.trim_count;
            }
            result->loops.push_back(loop_record);
            require(result->loops.size() <= limit, "Loop resource limit");
            ++record.loop_count;
        }
        result->faces.push_back(record);
    }
    // GetOrCreateUVTrimCurve can increase edge and vertex tolerances. Snapshot
    // them after every face trim has been created, so geometry and uncertainty
    // come from the same native state. Source geometry remains on a private copy.
    for (ULONG i = 0; i < vertices.GetSize(); ++i) {
        const auto p = vertices[i]->GetPoint();
        result->vertices.push_back({{p.x, p.y, p.z}, vertices[i]->GetTolerance()});
    }
    for (ULONG i = 0; i < edges.GetSize(); ++i) {
        const auto *edge = edges[i];
        const auto interval = edge->GetInterval();
        result->edges.push_back(
            {{vertex_index.at(edge->GetStartVertex()), vertex_index.at(edge->GetEndVertex())},
             {interval.GetMin(), interval.GetMax()},
             edge->GetTolerance(),
             result->keep(curve_payload(*static_cast<SmBSplineCurve *>(edge->GetCurve())))});
    }
    return result.release();
}
extern "C" int vb_brep_view(const VbBrep *brep, VbBrepView *view) {
    if (!brep || !view)
        return 1;
    *view = {brep->vertices.size(), brep->edges.size(),    brep->trims.size(), brep->loops.size(),
             brep->faces.size(),    brep->vertices.data(), brep->edges.data(), brep->trims.data(),
             brep->loops.data(),    brep->faces.data()};
    return 0;
}
extern "C" void vb_brep_free(VbBrep *brep) { delete brep; }
