#include "surface_fillet.h"
#include <SmApiGeneral.h>
#include <SmBrep.h>
#include <SmFace.h>
#include <SmEdge.h>
#include <SmVertex.h>
#include <SmFilletExecutive.h>
#include <SmSurface.h>
#include <algorithm>
#include <cmath>
#include <memory>
#include <stdexcept>
#include <string>

namespace {
void require(bool valid, const char *message) {
    if (!valid) throw std::runtime_error(message);
}
void check(SmStatus status, const char *operation) {
    if (status != SM_SUCCESS)
        throw std::runtime_error(std::string(operation) + " status " + std::to_string(status));
}
}

VbSurfaceFillet vb_fillet_surface_parents(const VbNurbsData &first, const VbNurbsData &second,
    double radius, const int sides[2], bool extend, double extension_factor, double accuracy, bool trim,
    bool retain_parents) {
    require(std::isfinite(radius) && radius > 0 && sides &&
                (sides[0] == 1 || sides[0] == -1) && (sides[1] == 1 || sides[1] == -1) &&
                std::isfinite(extension_factor) && extension_factor >= 0 &&
                std::isfinite(accuracy) && accuracy > SM_EFF_ZERO,
            "Invalid surface fillet radius, sides or accuracy");
    // Reserve precision for rail fitting, lofting and downstream projections.
    // Position tolerance alone can leave enough normal error to move an
    // off-surface closest point beyond the caller's model tolerance. Keep the
    // construction target tighter without changing caller acceptance limits.
    const double construction_accuracy = std::max(
        accuracy / 16.0, std::nextafter(SM_EFF_ZERO, INFINITY));
    const VbNurbsData *definitions[2] = {&first, &second};
    auto *context = SmApiGetOrCreateContext();
    VbSurfaceFillet output;
    auto &parents = output.parents;
    SmSurface *surfaces[2] = {};
    for (size_t i = 0; i < 2; ++i) {
        const auto &data = *definitions[i];
        require(data.degree[0] > 0 && data.degree[0] <= 17 &&
                    data.degree[1] > 0 && data.degree[1] <= 17,
                "Surface fillet degree limit");
        std::unique_ptr<SmSurface> surface(vb_import_query_surface(data));
        parents[i].reset(new (*context) SmBrep());
        parents[i]->SetTolerance(construction_accuracy, FALSE);
        auto *raw = surface.release();
        SmFace *face = nullptr;
        const auto status = parents[i]->CreateFaceFromSurface(raw, raw->GetNaturalUVDomain(), face);
        if (status != SM_SUCCESS && !raw->GetOwner()) surface.reset(raw);
        check(status, "create surface fillet parent");
        require(face, "Missing surface fillet parent face");
        surfaces[i] = face->GetSurface();
        if (retain_parents) {
            output.source_faces[i] = face;
            SmTArray<SmEdge *> edges;
            parents[i]->GetEdges(edges);
            for (ULONG j=0;j<edges.GetSize();++j) output.source_edges[i].push_back(edges[j]);
            SmTArray<SmVertex *> vertices;
            parents[i]->GetVertices(vertices);
            for (ULONG j=0;j<vertices.GetSize();++j) output.source_vertices[i].push_back(vertices[j]);
        }
    }
    // Topology insertion restores the incoming edit state. Its subsequent
    // face deletion needs these private parent copies to remain editable.
    if (trim) for (auto &parent : parents) parent->m_bEditingEnabled = TRUE;
    SmFilletExecutive executive(*context);
    SmBrep *raw = nullptr;
    // Beveled end caps retain each original rail endpoint when parent
    // lengths differ; minimal trimming keeps only their common extent.
    const auto status = executive.SurfaceSurfaceFillet(
        *context, surfaces[0], surfaces[1], sides[0] * radius, sides[1] * radius,
        construction_accuracy, raw, 2, 0.1, extend ? SM_BT_BEVEL : SM_BT_MINIMAL,
        trim ? 1 : 0, FALSE, FALSE, extension_factor);
    std::unique_ptr<SmBrep> result(raw);
    check(status, "fillet native surfaces");
    if (result) {
        SmTArray<SmFace *> faces;
        result->GetFaces(faces);
        require(faces.GetSize() > 0 && faces.GetSize() <= 1'000'000,
                "Invalid surface fillet face count");
    }
    for (auto &parent : parents) parent->m_bEditingEnabled = FALSE;
    output.fillet = std::move(result);
    return output;
}

bool vb_finish_fillet_parent(VbSurfaceFillet &result, size_t i) {
    require(i<2 && result.parents[i], "Missing surface fillet parent");
    auto &parent=result.parents[i];
    parent->m_bEditingEnabled=TRUE;
    // An open rail ending inside a parent does not divide its region. Remove
    // only embedded struts with the kernel's existing topology operations;
    // closed cuts, natural boundaries and periodic seams remain intact.
    size_t removed=0;
    for(;;) {
        SmTArray<SmEdge*> edges;
        parent->GetEdges(edges);
        SmEdge *strut=nullptr;
        for(ULONG j=0;j<edges.GetSize();++j)
            if(edges[j]->IsStrut() && edges[j]->IsEmbedded() && !edges[j]->IsClosed()) {
                strut=edges[j];break;
            }
        if(!strut) break;
        require(removed++<1'000'000,"Surface fillet parent cleanup limit");
        check(parent->DeleteEdge(strut),"remove nondividing fillet rail");
    }
    if(removed) {
        SmTArray<SmVertex*> vertices;
        parent->GetVertices(vertices);
        for(ULONG j=0;j<vertices.GetSize();++j) {
            auto *vertex=vertices[j];
            if(std::find(result.source_vertices[i].begin(),result.source_vertices[i].end(),vertex)
                !=result.source_vertices[i].end()) continue;
            SmTArray<SmEdge*> edges;
            vertex->GetEdges(edges);
            if(!edges.GetSize()) check(parent->DeleteVertex(vertex),"remove fillet imprint point");
        }
    }
    parent->m_bEditingEnabled=FALSE;
    SmTArray<SmFace*> faces;
    SmTArray<SmEdge*> edges;
    SmTArray<SmVertex*> vertices;
    parent->GetFaces(faces);parent->GetEdges(edges);parent->GetVertices(vertices);
    bool same=faces.GetSize()==1 && faces[0]==result.source_faces[i] &&
        edges.GetSize()==result.source_edges[i].size() &&
        vertices.GetSize()==result.source_vertices[i].size();
    for(ULONG j=0;same && j<edges.GetSize();++j)
        same=std::find(result.source_edges[i].begin(),result.source_edges[i].end(),edges[j])
            !=result.source_edges[i].end();
    for(ULONG j=0;same && j<vertices.GetSize();++j)
        same=std::find(result.source_vertices[i].begin(),result.source_vertices[i].end(),vertices[j])
            !=result.source_vertices[i].end();
    return !same;
}

SmBrep *vb_fillet_surfaces(const VbNurbsData &first, const VbNurbsData &second,
    double radius, const int sides[2], bool extend, double extension_factor, double accuracy) {
    return vb_fillet_surface_parents(first, second, radius, sides, extend,
        extension_factor, accuracy, false, false).fillet.release();
}
