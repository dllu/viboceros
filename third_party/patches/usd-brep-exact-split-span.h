#pragma once
#include <nurbs.h>
#include <algorithm>
#include <cmath>

// Split preserves the source's outer padding. An unclamped active endpoint
// can therefore produce an unused piece with an empty active domain. Outer
// vector endpoints themselves remain excluded by the existing contract.
static inline NL_FLAG sm_ExactSplitSpan(NL_KNOTVECTOR *knots, NL_DEGREE degree,
                                      NL_INDEX last_control, NL_PARAMETER parameter,
                                      NL_INDEX *span, NL_INDEX *multiplicity,
                                      const TCHAR *routine)
{
    const NL_INDEX last = knots->m;
    const NL_REAL *U = knots->U;
    if (degree < 0 || degree > NL_MAXDEG || last < 2 * degree + 1 ||
        last_control != last - degree - 1)
    {
        N_ErrSet(NL_INP_ERR, routine);
        return NL_YES;
    }
    if (!std::isfinite(parameter) || !(U[degree] < U[last - degree]) ||
        parameter < U[degree] || parameter > U[last - degree] ||
        parameter <= U[0] || parameter >= U[last])
    {
        N_ErrSet(NL_PAR_ERR, routine);
        return NL_YES;
    }
    const auto first = std::lower_bound(U, U + last + 1, parameter);
    const auto end = std::upper_bound(first, U + last + 1, parameter);
    *span = static_cast<NL_INDEX>(end - U) - 1;
    *multiplicity = static_cast<NL_INDEX>(end - first);
    if (*multiplicity > degree + 1)
    {
        N_ErrSet(NL_KNT_ERR, routine);
        return NL_YES;
    }
    return NL_NO;
}
