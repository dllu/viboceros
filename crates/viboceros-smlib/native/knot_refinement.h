#pragma once
#include "brep_data.h"
// Called under the shared kernel lock. No input data is modified.
VbSurfaceData *vb_refine_nurbs(const VbNurbsData &source, int axis, double parameter,
                              size_t target_multiplicity);
