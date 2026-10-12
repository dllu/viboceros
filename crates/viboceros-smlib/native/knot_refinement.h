#pragma once
#include "brep_data.h"
#include <stdexcept>
struct VbUnrepresentableNurbsControl : std::runtime_error {
    using std::runtime_error::runtime_error;
};
// Called under the shared kernel lock. No input data is modified.
VbSurfaceData *vb_refine_nurbs(const VbNurbsData &source, int axis, double parameter,
                              size_t target_multiplicity);
VbSurfaceData *vb_degree_nurbs(const VbNurbsData &source, int axis, size_t degree,
                              bool deformable, const double *knots, size_t knot_count);
VbSurfaceData *vb_fit_nurbs(const VbNurbsData &source, int axis,
                          const double *knots, size_t knot_count);
void vb_bezier_nurbs(const VbNurbsData &source,int direction,VbSurfaceData **output,size_t count);
