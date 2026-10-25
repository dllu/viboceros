#pragma once
#include <cstddef>
#include <cstdint>

// Euclidean XYZ/weight controls and a full knot vector. Output capacity is
// knot_count; only strictly interior discontinuities are returned.
extern "C" int32_t vibo_curve_discontinuities(uint32_t degree, size_t count,
    const double* controls, size_t knot_count, const double* knots,
    int32_t smooth, double cos_angle_tolerance, size_t* found, double* parameters,
    char* error, size_t capacity);
