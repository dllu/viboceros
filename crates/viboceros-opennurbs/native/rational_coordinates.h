#ifndef VIBOCEROS_RATIONAL_COORDINATES_H
#define VIBOCEROS_RATIONAL_COORDINATES_H

#include "opennurbs_public.h"
#include <algorithm>
#include <cmath>
#include <limits>
#include <string>

namespace vibo {

// Do not materialize 1/w: a finite Euclidean point can have a subnormal weight
// whose reciprocal overflows. Direct IEEE division handles this case.
inline bool EuclideanControl(const ON_4dPoint& source, ON_3dPoint& point) {
  if (!source.IsValid() || source.w == 0.0) return false;
  point.x = source.x / source.w;
  point.y = source.y / source.w;
  point.z = source.z / source.w;
  return point.IsValid();
}

// OpenNURBS stores homogeneous coordinates, but our ABI stores Euclidean
// coordinates plus weight. A common binary scale preserves every weight ratio
// exactly; no per-control rescaling or fitting is permitted.
inline bool HomogeneousControl(const double* point, int dimension, int shift,
                               double* result) {
  const double weight = point[dimension];
  const double scaled = std::scalbn(weight, shift);
  if (!ON_IsValid(scaled) || scaled == 0.0 ||
      std::scalbn(scaled, -shift) != weight) {
    return false;
  }
  // Public GetCV uses multiplication by the reciprocal, not direct division.
  const double reciprocal = 1.0 / scaled;
  if (!std::isfinite(reciprocal)) return false;
  for (int axis = 0; axis < dimension; ++axis) {
    const double coordinate = point[axis];
    if (!ON_IsValid(coordinate)) return false;
    result[axis] = coordinate * scaled;
    if (!ON_IsValid(result[axis])) return false;
    const auto recovers = [coordinate](double restored) {
      return restored == coordinate ||
          (ON_IsValid(restored) && coordinate != 0.0 && restored != 0.0 &&
           std::abs(restored / coordinate - 1.0) <=
               8.0 * std::numeric_limits<double>::epsilon());
    };
    // Check both our direct-division reader and Rhino's reciprocal getter.
    if (!recovers(result[axis] / scaled) || !recovers(result[axis] * reciprocal)) {
      return false;
    }
  }
  result[dimension] = scaled;
  return true;
}

// Input supplies Euclidean controls by index, allowing both the flat ABI and
// validated binary codecs to share the exact same conversion policy.
template <typename Input>
bool RationalScale(size_t count, int dimension, const Input& input, int& shift,
                   std::string& error) {
  auto valid = [&](int candidate) {
    double homogeneous[4] = {};
    for (size_t i = 0; i < count; ++i) {
      if (!HomogeneousControl(input(i), dimension, candidate, homogeneous))
        return false;
    }
    return true;
  };
  shift = 0;
  if (valid(0)) return true;  // Ordinary definitions remain bit-for-bit intact.

  int low = std::numeric_limits<int>::min();
  int high = std::numeric_limits<int>::max();
  int normal_low = low;
  int normal_high = high;
  int max_weight_exponent = low;
  for (size_t i = 0; i < count; ++i) {
    const double* point = input(i);
    const double weight = point[dimension];
    if (!std::isfinite(weight) || weight == 0.0) {
      error = "rational control has a nonfinite or zero weight";
      return false;
    }
    int weight_exponent = 0;
    const double weight_mantissa = std::frexp(weight, &weight_exponent);
    // Exponents below are floor(log2(abs(value))), including subnormals.
    --weight_exponent;
    max_weight_exponent = std::max(max_weight_exponent, weight_exponent);
    auto constrain = [&](int exponent, int minimum) {
      low = std::max(low, minimum - exponent);
      high = std::min(high, 1023 - exponent);
      normal_low = std::max(normal_low, -1022 - exponent);
      normal_high = std::min(normal_high, 1022 - exponent);
    };
    // Some values in the 2^-1024 bin still have finite reciprocals. Validate
    // the actual reciprocal rather than excluding the entire boundary bin.
    constrain(weight_exponent, -1024);
    for (int axis = 0; axis < dimension; ++axis) {
      if (!ON_IsValid(point[axis])) {
        error = "rational control coordinate is outside OpenNURBS range";
        return false;
      }
      if (point[axis] == 0.0) continue;
      int coordinate_exponent = 0;
      const double coordinate_mantissa = std::frexp(point[axis], &coordinate_exponent);
      const int product_exponent = weight_exponent + coordinate_exponent +
          std::ilogb(std::abs(weight_mantissa * coordinate_mantissa)) + 1;
      // Values above half the smallest subnormal round up to a nonzero value;
      // allow that bin only when the Euclidean round-trip check succeeds.
      constrain(product_exponent, -1075);
    }
  }
  const int preferred = -max_weight_exponent;
  if (normal_low <= normal_high) {
    shift = std::clamp(preferred, normal_low, normal_high);
    if (valid(shift)) return true;
  }
  // When no all-normal encoding exists, subnormal products can still be exact.
  // Validate each candidate rather than accepting rounded-away coordinates or
  // weights. This also covers the rounding bins at the subnormal boundary.
  if (low <= high) {
    shift = std::clamp(preferred, low, high);
    if (valid(shift)) return true;
    for (int candidate = low; candidate <= high; ++candidate) {
      if (candidate != shift && valid(candidate)) {
        shift = candidate;
        return true;
      }
    }
  }
  error = "rational controls have no lossless common binary weight scale for OpenNURBS";
  return false;
}

}  // namespace vibo
#endif
