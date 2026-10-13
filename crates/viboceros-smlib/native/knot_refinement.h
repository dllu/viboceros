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
VbSurfaceData *vb_trim_nurbs_curve(const VbNurbsData &source,double start,double end);
VbSurfaceData *vb_trim_nurbs_surface_axis(const VbNurbsData &source,int axis,double start,double end);
VbSurfaceData *vb_surface_isocurve(const VbNurbsData &source,int varying,double parameter);
struct VbInterpolationSystem {
    const double *knots,*parameters,*targets;
    size_t knot_count,parameter_count,columns,target_count;
    size_t degree;
    int closed,handles,uniform_periodic;
    const unsigned char *fixed_rows;
    size_t fixed_count;
    int point_grid;
    double *amplification;
    size_t amplification_count;
    // Retain nonuniform Greville phase and full pivoted storage for MakePeriodic.
    int nonuniform_periodic;
    size_t periodic_control_shift;
};
void vb_solve_interpolation(const VbInterpolationSystem &input,double *output,size_t output_count);
