#include "brep_data.h"
#include <SmApiGeneral.h>
#include <SmApiIntersectors.h>
#include <SmBSplineCurve.h>
#include <SmBSplineSurface.h>
#include <SmBrep.h>
#include <SmBrepData.h>
#include <SmExtent2d.h>
#include <SmFace.h>
#include <SmFaceuse.h>
#include <SmPointClass.h>
#include <SmRegion.h>
#include <SmShell.h>
#include <SmVertex.h>
#include <algorithm>
#include <cmath>
#include <limits>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

namespace {
constexpr size_t limit = 1'000'000;
void require(bool condition, const char *text) {
    if (!condition)
        throw std::runtime_error(text);
}
void check(SmStatus status, const char *operation) {
    if (status != SM_SUCCESS)
        throw std::runtime_error(std::string(operation) + " status " + std::to_string(status));
}
void knots(const VbNurbsData &data, int direction, SmTArray<double> &unique,
           SmTArray<ULONG> &multiplicities) {
    const size_t degree = data.degree[direction], count = data.count[direction],
                 n = data.knot_count[direction];
    require(degree >= 1 && degree <= 32 && count > degree && count <= limit &&
                n == count + degree + 1 && data.knots[direction],
            "Invalid NURBS dimensions");
    for (size_t i = 0; i < n; ++i) {
        const auto value = data.knots[direction][i];
        require(std::isfinite(value) && (i == 0 || value >= data.knots[direction][i - 1]),
                "Invalid NURBS knots");
        if (i == 0 || value != data.knots[direction][i - 1]) {
            unique.Add(value);
            multiplicities.Add(1);
        } else
            ++multiplicities[multiplicities.GetSize() - 1];
    }
    require(unique.GetSize() >= 2 && multiplicities[0] == degree + 1 &&
                multiplicities[multiplicities.GetSize() - 1] == degree + 1,
            "Import requires clamped NURBS");
    for (ULONG i = 1; i + 1 < multiplicities.GetSize(); ++i)
        require(multiplicities[i] <= degree, "Discontinuous NURBS interior knot");
}
void point(const double *p, SmTArray<SmPoint3d> &points, SmTArray<double> &weights) {
    require(std::isfinite(p[0]) && std::isfinite(p[1]) && std::isfinite(p[2]) &&
                std::isfinite(p[3]) && p[3] > 0,
            "Invalid NURBS controls");
    points.Add(SmPoint3d(p[0], p[1], p[2]));
    weights.Add(p[3]);
}
SmCurve *import_curve(const VbNurbsData &data, ULONG dimension) {
    require(data.points_xyzw && data.count[1] == 1, "Invalid curve payload");
    SmTArray<double> unique, weights;
    SmTArray<ULONG> multiplicities;
    SmTArray<SmPoint3d> points;
    knots(data, 0, unique, multiplicities);
    for (size_t i = 0; i < data.count[0]; ++i)
        point(data.points_xyzw + 4 * i, points, weights);
    SmBSplineCurve *raw = nullptr;
    const auto status = SmBSplineCurve::CreateCanonical(
        *SmApiGetOrCreateContext(), dimension, data.degree[0], points, SM_CF_UNSPECIFIED,
        multiplicities, unique, SM_KT_UNSPECIFIED, &weights, nullptr, raw);
    std::unique_ptr<SmBSplineCurve> owner(raw);
    check(status, "create imported curve");
    require(raw, "Missing imported curve");
    return owner.release();
}
SmSurface *import_surface(const VbNurbsData &data) {
    require(data.points_xyzw && data.count[1] && data.count[0] <= limit / data.count[1],
            "Invalid surface payload");
    SmTArray<double> u, v, weights;
    SmTArray<ULONG> mu, mv;
    SmTArray<SmPoint3d> points;
    knots(data, 0, u, mu);
    knots(data, 1, v, mv);
    for (size_t i = 0; i < data.count[0]; ++i)
        for (size_t j = 0; j < data.count[1]; ++j)
            point(data.points_xyzw + 4 * (j * data.count[0] + i), points, weights);
    SmBSplineSurface *raw = nullptr;
    const auto status = SmBSplineSurface::CreateCanonical(
        *SmApiGetOrCreateContext(), data.degree[0], data.degree[1], points, SM_SF_UNSPECIFIED, mu,
        mv, u, v, SM_KT_UNSPECIFIED, &weights, nullptr, raw);
    std::unique_ptr<SmBSplineSurface> owner(raw);
    check(status, "create imported surface");
    require(raw, "Missing imported surface");
    return owner.release();
}
} // namespace

SmBrep *vb_import_brep(const VbBrepView &view, double tolerance, const size_t *components,
                       size_t component_count, const int *inward,
                       std::vector<size_t> *parents, std::vector<SmEdge *> *source_edges) {
    require(std::isfinite(tolerance) && tolerance > 0, "Invalid import tolerance");
    require(view.vertex_count && view.edge_count && view.face_count && view.vertices &&
                view.edges && view.faces && view.loops && view.trims,
            "Missing B-rep data");
    require(view.vertex_count <= limit && view.edge_count <= limit && view.face_count <= limit &&
                view.loop_count <= limit && view.trim_count <= limit,
            "B-rep import resource limit");
    SmBrepData data(TRUE, SM_DS_OPENNURBS);
    data.m_sZoneTol3d = tolerance;
    require(components && inward && component_count && component_count <= view.face_count,
            "Invalid component map");
    require(component_count <= 128, "B-rep component nesting work limit");
    for (size_t f = 0; f < view.face_count; ++f)
        require(components[f] < component_count, "Invalid face component index");
    data.m_lNumRegions = component_count + 1;
    data.m_lHealerVersion = SM_HEALER_VERSION;
    data.m_vRegions.SetSize(component_count + 1);
    data.m_vShells.SetSize(component_count * 2);
    data.m_vRegions[0].m_lStartShell = 0;
    data.m_vRegions[0].m_lNumShells = component_count;
    data.m_vRegions[0].m_bIsVoidFlag = TRUE;
    for (size_t c = 0; c < component_count; ++c) {
        auto &region = data.m_vRegions[c + 1];
        region.m_lStartShell = component_count + c;
        region.m_lNumShells = 1;
        region.m_bIsVoidFlag = FALSE;
        for (size_t side = 0; side < 2; ++side) {
            auto &shell = data.m_vShells[c + side * component_count];
            shell.m_lShellType = 0;
            shell.m_lFaceuseStart = data.m_vFaceuses.GetSize();
            for (size_t f = 0; f < view.face_count; ++f)
                if (components[f] == c) {
                    SmFaceuseData faceuse;
                    faceuse.m_lFace = f;
                    faceuse.m_bOrientation =
                        ((side == 0) != bool(view.faces[f].reversed)) != bool(inward[c]);
                    data.m_vFaceuses.Add(faceuse);
                    ++shell.m_lNumFaceuses;
                }
            require(shell.m_lNumFaceuses > 0, "Empty imported shell");
        }
    }
    data.m_vVertices.SetSize(view.vertex_count);
    for (size_t i = 0; i < view.vertex_count; ++i) {
        const auto &v = view.vertices[i];
        require(std::isfinite(v.point[0]) && std::isfinite(v.point[1]) &&
                    std::isfinite(v.point[2]) && std::isfinite(v.tolerance) && v.tolerance >= 0,
                "Invalid imported vertex");
        data.m_vVertices[i].m_vPoint = SmPoint3d(v.point[0], v.point[1], v.point[2]);
        data.m_vVertices[i].m_sZoneTol3d = v.tolerance;
    }
    data.m_vEdges.SetSize(view.edge_count);
    for (size_t i = 0; i < view.edge_count; ++i) {
        const auto &edge = view.edges[i];
        require(edge.vertices[0] < view.vertex_count && edge.vertices[1] < view.vertex_count,
                "Missing imported edge vertex");
        auto &e = data.m_vEdges[i];
        e.m_lCurve = data.m_v3DCurves.GetSize();
        e.m_lStartVertex = edge.vertices[0];
        e.m_lEndVertex = edge.vertices[1];
        require(std::isfinite(edge.tolerance) && edge.tolerance >= 0 &&
                    std::isfinite(edge.interval[0]) && std::isfinite(edge.interval[1]) &&
                    edge.interval[0] < edge.interval[1],
                "Invalid edge interval or tolerance");
        e.m_sZoneTol3d = edge.tolerance;
        e.m_vInterval.SetMinMax(edge.interval[0], edge.interval[1]);
        data.m_v3DCurves.Add(import_curve(edge.curve, 3));
    }
    data.m_vFaces.SetSize(view.face_count);
    data.m_vLoops.SetSize(view.loop_count);
    data.m_vEdgeuses.SetSize(view.trim_count);
    std::vector<std::vector<size_t>> uses(view.edge_count);
    std::vector<bool> seen_loops(view.loop_count, false), seen_trims(view.trim_count, false);
    std::vector<size_t> trim_faces(view.trim_count);
    for (size_t f = 0; f < view.face_count; ++f) {
        const auto &face = view.faces[f];
        require(face.first_loop <= view.loop_count &&
                    face.loop_count <= view.loop_count - face.first_loop,
                "Missing imported face loop");
        require(face.loop_count > 0 && (face.reversed == 0 || face.reversed == 1),
                "Invalid face loops or orientation");
        data.m_vSurfaces.Add(import_surface(face.surface));
        auto &record = data.m_vFaces[f];
        record.m_lSurface = f;
        record.m_lStartLoop = face.first_loop;
        record.m_lNumLoops = face.loop_count;
        record.m_vUVDomain = data.m_vSurfaces[f]->GetNaturalUVDomain();
        for (size_t l = face.first_loop; l < face.first_loop + face.loop_count; ++l) {
            require(!seen_loops[l], "Loop is shared by imported faces");
            seen_loops[l] = true;
            const auto &loop = view.loops[l];
            require(loop.trim_count > 0 && (loop.inner == 0 || loop.inner == 1) &&
                        (l != face.first_loop || !loop.inner),
                    "Invalid loop orientation");
            require(loop.first_trim <= view.trim_count &&
                        loop.trim_count <= view.trim_count - loop.first_trim,
                    "Missing imported trim");
            auto &record = data.m_vLoops[l];
            record.m_lLoopType = 0;
            record.m_lStartEU = loop.first_trim;
            record.m_lNumEU = loop.trim_count;
            for (size_t t = loop.first_trim; t < loop.first_trim + loop.trim_count; ++t) {
                require(!seen_trims[t], "Trim is shared by imported loops");
                seen_trims[t] = true;
                trim_faces[t] = f;
                const auto &trim = view.trims[t];
                require(trim.vertices[0] < view.vertex_count &&
                            trim.vertices[1] < view.vertex_count &&
                            (trim.reversed == 0 || trim.reversed == 1),
                        "Invalid trim vertices or orientation");
                auto &e = data.m_vEdgeuses[t];
                e.m_lLoop = l;
                if (trim.edge == std::numeric_limits<size_t>::max()) {
                    require(trim.vertices[0] == trim.vertices[1] &&
                                trim.vertices[0] < view.vertex_count,
                            "Invalid imported pole trim");
                    e.m_lEUType = 3;
                    e.m_lVertex = trim.vertices[0];
                    continue;
                }
                require(trim.edge < view.edge_count, "Missing imported trim edge");
                const auto &endpoints = view.edges[trim.edge].vertices;
                require(trim.vertices[0] == endpoints[trim.reversed ? 1 : 0] &&
                            trim.vertices[1] == endpoints[trim.reversed ? 0 : 1],
                        "Trim disagrees with edge direction");
                e.m_lEUType = 0;
                e.m_lEdge = trim.edge;
                e.m_bOrientation = !trim.reversed;
                e.m_lUVCurve = data.m_vUVCurves.GetSize();
                data.m_vUVCurves.Add(import_curve(trim.curve, 2));
                uses[trim.edge].push_back(t);
            }
        }
    }
    require(std::all_of(seen_loops.begin(), seen_loops.end(), [](bool v) { return v; }) &&
                std::all_of(seen_trims.begin(), seen_trims.end(), [](bool v) { return v; }),
            "Unowned imported loop or trim");
    for (size_t i = 0; i < uses.size(); ++i) {
        require(uses[i].size() == 2, "Import requires closed manifold edges");
        const auto a = uses[i][0], b = uses[i][1];
        require(components[trim_faces[a]] == components[trim_faces[b]],
                "Edge crosses component map");
        const bool first = bool(view.trims[a].reversed) != bool(view.faces[trim_faces[a]].reversed);
        const bool second =
            bool(view.trims[b].reversed) != bool(view.faces[trim_faces[b]].reversed);
        require(first != second, "Invalid directed manifold edge incidence");
        data.m_vEdges[i].m_lPrimEU = uses[i][0];
    }
    for (ULONG i = 0; i < data.m_vRegions.GetSize(); ++i)
        data.m_vRegions[i].m_lFlags = 0;
    for (ULONG i = 0; i < data.m_vShells.GetSize(); ++i)
        data.m_vShells[i].m_lFlags = 0;
    for (ULONG i = 0; i < data.m_vFaceuses.GetSize(); ++i)
        data.m_vFaceuses[i].m_lFlags = 0;
    for (ULONG i = 0; i < data.m_vFaces.GetSize(); ++i)
        data.m_vFaces[i].m_lFlags = 0;
    for (ULONG i = 0; i < data.m_vLoops.GetSize(); ++i)
        data.m_vLoops[i].m_lFlags = 0;
    for (ULONG i = 0; i < data.m_vEdges.GetSize(); ++i)
        data.m_vEdges[i].m_lFlags = 0;
    for (ULONG i = 0; i < data.m_vEdgeuses.GetSize(); ++i)
        data.m_vEdgeuses[i].m_lFlags = 0;
    for (ULONG i = 0; i < data.m_vVertices.GetSize(); ++i)
        data.m_vVertices[i].m_lFlags = 0;
    auto result = std::unique_ptr<SmBrep>(new (*SmApiGetOrCreateContext()) SmBrep);
    SmTArray<SmAttribute *> attributes;
    check(result->MakeTopologyFromData(&data, attributes, FALSE, FALSE, TRUE, FALSE, FALSE, FALSE),
          "build imported topology");
    require(result->IsManifoldSolid(), "Imported topology is not a manifold solid");
    std::vector<size_t> shell_parents(component_count, SIZE_MAX);
    if (component_count > 1) {
        std::vector<std::unique_ptr<SmBrep>> parts;
        std::vector<SmRegion *> regions(component_count);
        for (size_t c = 0; c < component_count; ++c) {
            auto part = std::unique_ptr<SmBrep>(new (*SmApiGetOrCreateContext()) SmBrep);
            SmTArray<SmFace *> selected;
            for (size_t f = 0; f < view.face_count; ++f)
                if (components[f] == c)
                    selected.Add(data.m_vFaces[f].m_pFace);
            SmTArray<SmFace *> copied;
            check(result->CopyFaces(selected, part.get(), &copied, TRUE),
                  "copy shell for containment");
            parts.push_back(std::move(part));
            regions[c] = data.m_vRegions[c + 1].m_pRegion;
        }
        std::vector<std::vector<bool>> inside(component_count,
                                              std::vector<bool>(component_count, false));
        size_t face_pairs = 0;
        for (size_t a = 0; a < component_count; ++a)
            for (size_t b = a + 1; b < component_count; ++b) {
                SmTArray<SmFace *> first, second;
                parts[a]->GetFaces(first);
                parts[b]->GetFaces(second);
                for (ULONG i = 0; i < first.GetSize(); ++i)
                    for (ULONG j = 0; j < second.GetSize(); ++j) {
                        require(++face_pairs <= 65536, "Component intersection work limit");
                        SmTArray<SmCurve *> curves;
                        SmTArray<SmPoint3d> points;
                        const auto status =
                            SmApiIntersectFaces(first[i], second[j], curves, &points);
                        const bool intersected = curves.GetSize() != 0 || points.GetSize() != 0;
                        for (ULONG k = 0; k < curves.GetSize(); ++k)
                            delete curves[k];
                        check(status, "check imported component face intersections");
                        require(!intersected, "Imported shell components intersect or touch");
                    }
            }
        for (size_t c = 0; c < component_count; ++c)
            for (size_t p = 0; p < component_count; ++p)
                if (c != p) {
                    SmTArray<SmVertex *> vertices;
                    parts[c]->GetVertices(vertices);
                    bool classified = false, value = false;
                    for (ULONG v = 0; v < vertices.GetSize(); ++v) {
                        SmPointClassification classification;
                        check(parts[p]->Point3DClassify(vertices[v]->GetPoint(), tolerance, TRUE,
                                                        classification),
                              "classify imported shell nesting");
                        require(classification.GetPointClass() == SM_PC_REGION,
                                "Imported shell components touch or cannot be classified");
                        const auto *region = static_cast<SmRegion *>(classification.GetObject());
                        require(region, "Missing containment region");
                        const bool current = !region->IsInfiniteRegion();
                        require(!classified || current == value,
                                "Imported shell components intersect");
                        value = current;
                        classified = true;
                    }
                    require(classified, "No containment witness");
                    inside[c][p] = value;
                }
        for (size_t c = 0; c < component_count; ++c) {
            size_t parent = component_count;
            size_t depth = 0;
            for (size_t p = 0; p < component_count; ++p)
                if (inside[c][p]) {
                    ++depth;
                    require(!inside[p][c], "Cyclic component containment");
                    if (parent == component_count || inside[p][parent])
                        parent = p;
                }
            require(bool(inward[c]) == bool(depth % 2),
                    "Shell orientation disagrees with material nesting");
            shell_parents[c] = parent == component_count ? SIZE_MAX : parent;
            if (parent != component_count) {
                auto *shell = data.m_vShells[c].m_pShell1;
                check(result->GetInfiniteRegion()->Remove(shell), "detach nested shell");
                check(regions[parent]->PostInsert(shell), "attach nested shell");
            }
        }
        check(result->SetRegionIsVoidFlagsForNestedSolids(), "classify nested material regions");
    } else
        require(!inward[0], "Imported outer shell points inward");
    if (parents)
        *parents = std::move(shell_parents);
    if (source_edges) {
        source_edges->reserve(view.edge_count);
        for (size_t i = 0; i < view.edge_count; ++i)
            source_edges->push_back(data.m_vEdges[i].m_pEdge);
    }
    return result.release();
}
