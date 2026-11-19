#pragma once

#include <SmCurveTypes.h>

// The three upstream enumerators leave 3 inside the C++ enum's range.
// Only the patched offset join and the draft bridge use this value.
static_assert(SM_OC_LINEAR_CHAMFER == 2);
inline constexpr auto vb_smooth_taper_corner = static_cast<SmOffsetCornerType>(3);
