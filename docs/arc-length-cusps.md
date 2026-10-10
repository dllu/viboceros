# Arc length at turnarounds

The previous recursive integrator divided the absolute tolerance between every
child and applied relative tolerance to each child separately. A non-dyadic cusp
exhausted its depth limit, even when the whole integral could meet the requested
error. Signed cancellation could also return a result outside the global
absolute tolerance: one regression returned about -9.94e-12 when 1e-12 was asked.

Adaptive Gauss-Kronrod integration now tracks the sum of estimated errors across
the active partition. A heap selects the interval with the largest error;
acceptance uses the requested absolute tolerance or relative tolerance times the
total integral. Exact finite accumulators add and remove interval estimates
without losing small terms through cancellation. A smooth integral accepted by
the initial rule still returns without allocating a refinement heap.

An interval with stable child estimates at its roundoff floor is retained in
the global sums and removed from the refinement queue. Other intervals can
continue refining. Unattainable precision, unrepresentable midpoints and the
65,536 evaluated-interval budget return errors. The estimates remain numerical
quadrature estimates, not a certificate for arbitrary unsampled features.

Independent regressions check the integral of |x-0.4|, signed cancellation,
bounded failure at roundoff, and continued refinement beside a large constant
interval. A quadratic backtracking NURBS has exact total variation 3.25;
length and count stations now agree in ordinary, shifted, tiny and huge domains.

[Eighteen licensed Rhino recipes](../tools/rhino_oracle/fixtures/divide_turnaround.json)
exercise planar and spatial turnarounds, count/length points, Split, endpoint
marking and remainder deletion under private Xvfb. Local commands previously
failed all eighteen; they now succeed on all eighteen. Fifteen match at 5e-7
absolute plus 1e-12 relative epsilon. None match every field at 1e-9 absolute
epsilon. The [full comparison](divide-turnaround-comparison.json) retains three
cusp inversion diagnostics: both Split cases and the spatial cusp point case.
Native cut parameters differ near a zero-speed station; those differences are
not resolved by the length fix and are not used to fit the implementation.

[Provenance](arc-length-cusp-provenance.json) records sources and final validation.
Sharp unsampled features, extreme rational weight ratios, general native cusp
inversion and full geometry/performance qualification remain open.
