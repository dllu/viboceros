#include "surface_fillet.h"
#include <SmApiGeneral.h>
#include <SmBrep.h>
#include <SmFace.h>
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

SmBrep *vb_fillet_surfaces(const VbNurbsData &first, const VbNurbsData &second,
                          double radius, const int sides[2], bool extend, double extension_factor, double accuracy) {
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
    std::unique_ptr<SmBrep> parents[2];
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
    }
    SmFilletExecutive executive(*context);
    SmBrep *raw = nullptr;
    // Beveled end caps retain each original rail endpoint when parent
    // lengths differ; minimal trimming keeps only their common extent.
    const auto status = executive.SurfaceSurfaceFillet(
        *context, surfaces[0], surfaces[1], sides[0] * radius, sides[1] * radius,
        construction_accuracy, raw, 2, 0.1, extend ? SM_BT_BEVEL : SM_BT_MINIMAL,
        0, FALSE, FALSE, extension_factor);
    std::unique_ptr<SmBrep> result(raw);
    check(status, "fillet native surfaces");
    if (result) {
        SmTArray<SmFace *> faces;
        result->GetFaces(faces);
        require(faces.GetSize() > 0 && faces.GetSize() <= 1'000'000,
                "Invalid surface fillet face count");
    }
    return result.release();
}
