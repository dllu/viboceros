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
std::vector<double> full_knots(const SmTArray<double> &unique,
                               const SmTArray<ULONG> &multiplicities) {
    require(unique.GetSize() == multiplicities.GetSize(), "Invalid native knot multiplicities");
    std::vector<double> knots;
    for (ULONG i = 0; i < unique.GetSize(); ++i) {
        require(std::isfinite(unique[i]) && multiplicities[i] <= 65 &&
                    knots.size() + multiplicities[i] <= limit,
                "Invalid native knots");
        knots.insert(knots.end(), multiplicities[i], unique[i]);
    }
    return knots;
}
std::unique_ptr<NurbsPayload> curve_payload(const SmBSplineCurve &curve) {
    auto data = std::make_unique<NurbsPayload>();
    SmTArray<SmPoint3d> points;
    SmTArray<double> weights, knots;
    SmTArray<ULONG> multiplicities;
    check(curve.GetControlPolygon(points, weights), "export curve controls");
    require(points.GetSize() <= limit &&
                (weights.GetSize() == 0 || points.GetSize() == weights.GetSize()),
            "Invalid curve controls");
    for (ULONG i = 0; i < points.GetSize(); ++i) {
        const auto &p = points[i];
        data->points.insert(data->points.end(),
                            {p.x, p.y, p.z, weights.GetSize() ? weights[i] : 1.});
    }
    check(curve.GetKnots(knots, &multiplicities), "export curve knots");
    data->knots[0] = full_knots(knots, multiplicities);
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
    for (int d = 0; d < 2; ++d) {
        const auto direction = d == 0 ? SM_SP_U : SM_SP_V;
        SmTArray<double> knots;
        SmTArray<ULONG> multiplicities;
        check(surface.GetKnots(direction, knots, &multiplicities), "export surface knots");
        data->knots[d] = full_knots(knots, multiplicities);
        data->view.degree[d] = surface.GetDegree(direction);
    }
    data->view.count[0] = nu;
    data->view.count[1] = nv;
    data->finish();
    return data;
}
} // namespace

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

VbBrep *vb_export_brep(const SmBrep &source) {
    SmBrep *raw = nullptr;
    const auto status = SmApiBrepCopy(&source, raw);
    std::unique_ptr<SmBrep> copy(raw);
    check(status, "copy for B-rep export");
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
    for (ULONG i = 0; i < vertices.GetSize(); ++i) {
        vertex_index.emplace(vertices[i], i);
        const auto p = vertices[i]->GetPoint();
        result->vertices.push_back({{p.x, p.y, p.z}, vertices[i]->GetTolerance()});
    }
    for (ULONG i = 0; i < edges.GetSize(); ++i) {
        const auto *edge = edges[i];
        edge_index.emplace(edge, i);
        const auto interval = edge->GetInterval();
        result->edges.push_back(
            {{vertex_index.at(edge->GetStartVertex()), vertex_index.at(edge->GetEndVertex())},
             {interval.GetMin(), interval.GetMax()},
             edge->GetTolerance(),
             result->keep(curve_payload(*static_cast<SmBSplineCurve *>(edge->GetCurve())))});
    }
    for (ULONG i = 0; i < faces.GetSize(); ++i) {
        auto *face = faces[i];
        SmFaceuse *positive = nullptr;
        SmFaceuse *negative = nullptr;
        face->GetFaceuses(positive, negative);
        if (positive->GetOrientation() != SM_OT_SAME)
            std::swap(positive, negative);
        require(positive->GetRegion() && negative->GetRegion(), "Missing face region");
        require(positive->GetRegion()->IsVoid() != negative->GetRegion()->IsVoid(),
                "Export requires a material boundary");
        // Loops follow the positive surface side; the face flip follows the void side.
        VbFaceData record{
            result->keep(surface_payload(*static_cast<SmBSplineSurface *>(face->GetSurface()))),
            result->loops.size(), 0, !positive->GetRegion()->IsVoid()};
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
                const auto a = vertex_index.at(edge->GetStartVertex()),
                           b = vertex_index.at(edge->GetEndVertex());
                result->trims.push_back({{reversed ? b : a, reversed ? a : b},
                                         edge_index.at(edge),
                                         reversed,
                                         result->keep(curve_payload(*uv))});
                require(result->trims.size() <= limit, "Trim resource limit");
                ++loop_record.trim_count;
            }
            result->loops.push_back(loop_record);
            require(result->loops.size() <= limit, "Loop resource limit");
            ++record.loop_count;
        }
        result->faces.push_back(record);
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
