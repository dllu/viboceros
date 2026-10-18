#pragma once
#include "brep_data.h"
class SmBrep;
// Call under the process kernel lock. Inputs are imported into private parents;
// the returned fillet owns its geometry, or is null when no rail was found.
SmBrep *vb_fillet_surfaces(const VbNurbsData &first, const VbNurbsData &second,
                          double radius, const int sides[2], bool extend, double extension_factor, double accuracy);
