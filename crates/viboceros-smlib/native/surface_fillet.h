#pragma once
#include "brep_data.h"
#include <array>
#include <memory>
class SmBrep;
class SmFace;
class SmEdge;
class SmVertex;
struct VbSurfaceFillet {
    std::unique_ptr<SmBrep> fillet;
    std::array<std::unique_ptr<SmBrep>, 2> parents;
    // Identity comparisons only after insertion: deleted originals are never read.
    std::array<SmFace *, 2> source_faces{};
    std::array<std::vector<SmEdge *>, 2> source_edges;
    std::array<std::vector<SmVertex *>, 2> source_vertices;
};
// Parent topology is imprinted at the rails; trim selects the outside remnant.
VbSurfaceFillet vb_fillet_surface_parents(const VbNurbsData &first, const VbNurbsData &second,
    double radius, const int sides[2], bool extend, double extension_factor, double accuracy, bool trim,
    bool retain_parents=true);
// Remove nondividing imprints; true means the parent requires replacement.
bool vb_finish_fillet_parent(VbSurfaceFillet &result, size_t index);
// Call under the process kernel lock. Inputs are imported into private parents;
// the returned fillet owns its geometry, or is null when no rail was found.
SmBrep *vb_fillet_surfaces(const VbNurbsData &first, const VbNurbsData &second,
                          double radius, const int sides[2], bool extend, double extension_factor, double accuracy);
