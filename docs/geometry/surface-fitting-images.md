# Fitting complete surface images

[UV-preserving fitting](surface-fitting-automatic.md) · [Explicit bases](surface-fitting-mapped.md)

`try_fit_surface_image_with_bound(source, options, limit)` returns polynomial
geometry, a complete symmetric Hausdorff bound on its untrimmed image, and a
retained `SurfaceImageFitCertificate`. Source domains remain unchanged;
parameter speed can change. Zero is strict. The separate
`try_fit_surface_with_bound` API retains its normalized UV accuracy contract.

Each polynomial candidate first receives ordinary parameter admission. On the
first decline, the image fitter also tries a bilinear corner patch elevated to
the requested degrees, admitted through the complete planar quad certificate.
This can remove rational speed changes and folds when the full image is covered
by the same convex quad. Source boundaries and interiors participate in the proof.

Remaining candidates can receive independent monotone U/V maps proposed from
closest points on their mid-isocurves. Owned unit-domain curves propose clocks;
all accuracy checks refer to the original stored tensors. Prepared curve
queries and successful station parameters are reused across grids of 4, 8 and
16 divisions. Each axis has at most 17 pairs. A floating grid containing map
stations and midpoints rejects unsuitable proposals, using streamed tensor
contractions. Original source and candidate knot crossings enrich that grid
when it stays within 16,384 points; larger knot grids retain the basic 33×33
maximum. Samples never authorize a fit.

Passing proposals require the complete [mapped image certificate](surface-mapped-images.md),
including every original knot crossing and both one-sided limits. Failed,
nonmonotone or numerically unavailable clock proposals continue the ordinary
parameter-guided basis refinement. Coupled U/V warps and optimal maps remain
unfinished. The existing parameter sampling cache stays capped at 262,144
points; the additional map proposals have their own bounded grids per candidate.

`BoundedSurfaceImageFit::certificate` records `NormalizedUv`, `PlanarQuad` or
`SeparableUv` with the actual normalized parameter pairs. Calling
`certificate.verify(source, candidate, limit)` rechecks the complete image
accuracy. The explicit-basis image fitter retains its supplied maps too.
This permits auditing or repeating the check after a 3DM round-trip.

Both automatic fitters share `SurfaceFitOptions`, source limits, degree/control
ceilings, input validation and backend error handling. Inputs remain immutable.
`None` means no proposal was certified within the configured limits. No trim,
tangent continuity, topology or injectivity claim is made. The `_with_solver`
variant and `viboceros_smlib::fit_surface_image_with_bound` share admission and
the batched Cartesian collocation adapter; maps require no extra collocation solves.

The local JSON operation `surface_fit_image_bounded` accepts the same inputs as
`surface_fit_bounded`. It additionally returns `certificate`: a `type` of
`normalized_uv`, `planar_quad` or `separable_uv`; the latter includes
`correspondence: [u_pairs,v_pairs]`. Declined fits return null fields. The UV
operation retains its existing response. Mapped pairs can be passed directly
to `surface_mapped_deviation_bound` for a complete independent recheck.

The [current validation](../surface-image-auto.md) replays all
112 saved public Rhino fits, rechecks each returned source certificate and
checks complete 3DM round-trips. Native output agreement retains the separate
`1e-9` gate; a requested-tolerance image bound is a different claim. Native nulls
and inconclusive comparisons remain explicit. The earlier
[quad-only record](../surface-image-fit-validation.json) retains its original
93 outputs and 54 native image matches as historical evidence.
Rhino's adaptive fitting policy and the physical `FitSrf` command remain unfinished.
