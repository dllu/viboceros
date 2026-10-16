#pragma once
#include "brep_data.h"

// Call under the shared bridge lock. Inputs use a common unit parameter chart
// and a private translated/scaled geometry frame. The payload owns its data.
VbSurfaceData *vb_fit_network_curve(const double *points, const double *parameters,
    size_t point_count, const size_t *fixed, size_t fixed_count,
    const double *knots, size_t knot_count, const double *end_derivatives,
    int refine, double tolerance, double *maximum_error);
