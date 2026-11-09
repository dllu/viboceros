# Fitting complete surface images

[UV-preserving fitting](surface-fitting-automatic.md) · [Planar image proof](surface-planar-images.md)

`try_fit_surface_image_with_bound(source, options, limit)` returns a
`BoundedSurfaceImageFit`: polynomial geometry and a complete symmetric
Hausdorff distance bound on its untrimmed image. UV domains remain unchanged;
parameter correspondence can change. Zero is strict. The existing
`try_fit_surface_with_bound` API continues to guarantee normalized UV accuracy.

[Explicit-basis image fitting](surface-fitting-mapped.md) separately admits
curved fits through supplied U/V correspondences.

The image fitter first tries the ordinary one-patch parameter fit. If its
complete parameter certificate passes, that result is retained. Otherwise it
tries a bilinear corner patch represented at the requested U/V degrees, with
`degree+1` controls in each direction. The actual floating-point candidate is
accepted only through the exact auxiliary-quad image certificate. A failed or
inconclusive image proof continues the existing adaptive parameter fitter,
without repeating its initial proposal or backend solves.

The quad proposal can remove nonlinear or rational parameter speed, and even
folds, when the entire source image is covered by the same convex quad.
Nearby nonplanar features may receive conservative positive image bounds.
Source interiors and all boundaries participate in the proof; four matching
edges or sampled points alone cannot authorize a fit. The quad proof declines
discontinuous and unclamped sources, real overshoot and unsupported corner quads.
The ordinary parameter fallback can still fit such inputs when certifiable.

Both fitters share `SurfaceFitOptions`, source limits, degree/control ceilings,
validation and backend error handling. Inputs remain immutable. `None` means
no candidate was certified within the configured limits; it is not proof of
impossibility. Neither API qualifies trimming, tangent continuity or topology.

The `_with_solver` variant and
`viboceros_smlib::fit_surface_image_with_bound` use the shared batched Cartesian
collocation path. Analytic bilinear controls require no extra linear solve.

The local JSON operation `surface_fit_image_bounded` has the same inputs and
outputs as `surface_fit_bounded`: source definition, degrees, optional
`maximum_control_count` (default `[64,64]`) and `absolute_tolerance` (default
request tolerance); output definition, complete bound and unchanged source
snapshots. The two operation names specify different accuracy contracts.

The [validation record](../surface-image-fit-validation.json) compares both
APIs against all 112 archived public Rhino `Surface.Fit` calls. Each successful
output receives a complete 3DM round-trip bound at `1e-9`. Native parameter and
quad image bounds are recorded separately at that same strict gate; native null
results remain explicit. Full Rhino fitting policy and the physical `FitSrf`
command remain unfinished. No new desktop GUI sessions are needed for replay.

At `[16,16]` ceilings, both builds certify 93 outputs and 54 complete native
image matches. The UV fitter retains 87 outputs and 35 parameter matches, with
48 image matches when the quad certificate is checked separately. Image fitting
recovers six declined planar cases and reduces control counts in thirteen;
all 30 planar cases fit in one patch and match the native images at `1e-9`.
The ledger retains 27 unproved native images, 12 local fits with native null,
nine native surfaces without local fits and ten shared declines.
