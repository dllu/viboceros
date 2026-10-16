# Gordon curve-network foundation

The optional native kernel exposes
`viboceros_smlib::gordon_network_surface(curves, edge_tolerance, interior_tolerance)`.
It constructs a surface through two crossing curve families using usd-brep's
existing Gordon implementation. Rhino-compatible `NetworkSrf` fitting remains
in development; the application does not register that command yet.

The adapter discovers the families from curve intersections, sorts their
crossings and aligns their directions. Inputs retain their complete original
geometry and parameter domains. Construction uses a translated, scaled frame
and private native curves. Existing native intersections supply the original
crossing stations. `SmBSplineCurve::ApproximateRationalCurve` converts rational
inputs; native knot insertion, explicit-tolerance compatibility and
`N_CreateGordonSrf` construct the tensor. Native stack owners and the existing
bridge mutex protect lifetimes and kernel state.

The adapter aligns intersection stations with native knot insertion and
piecewise affine knot mapping, rejecting collapsed stored spans. The general
composition helper deliberately pushes knots near mapping stations by one
percent of a span, which displaced crossing-constrained refits outside their
requested tolerance. The bounded affine path preserves their images instead.
Family bases become compatible only after alignment, using the existing
explicit-tolerance operation with `1e-300`; the default operation groups knots
at one-thousandth of the domain. Reusing the original intersection graph also
avoids a second intersection search. An initial family union would needlessly
multiply the aligned knot counts and exhaust the surface proof budget.
Full native output knot arrays survive surface transfer through the existing
basis-restoration helper. The tensor allocation guard bounds degree elevation
and the union of the three Gordon component bases before construction.

Rational approximation receives a fraction of the smaller requested curve
tolerance, with a numerical floor of `1e-12` in the working frame. That floor
does not relax acceptance: every original input curve must subsequently have a
continuous image bound at its requested tolerance in the original world frame.
An inconclusive proof rejects the construction.

The kernel's exported station parameters and independently validated original
crossings propose piecewise linear UV traces. The existing exact-rational and
outward-rounded surface/curve certificate checks the complete traces. The
certified general pullback remains a fallback for other correspondences.
`NurbsSurface::prepare_surface_curve_bounds` and the public
`PreparedSurfaceCurveBounds::bound` reuse the existing exact surface cache across
independent proofs; each proof retains its fresh work budget and charges the
initial extraction work. Sampled distances alone never admit output.

Current construction limits are:

- Two families of 2–32 curves, 4–64 total; degree at most 16, at most 128
  controls per input and 2,048 total input controls.
- Open, noncollapsed, continuous curves with common-sign weights supported by
  the shared native importer. Every opposite-family pair must have one
  unambiguous contact; same-family contacts and overlaps are rejected.
- Complete curves must terminate at the outer-family crossings. Closed
  networks, partial curves, poles shared within a family, and unresolved
  multiple contacts need further preparation.
- Native prepared curves are limited to 2,048 controls. A conservative
  compatibility bound precedes tensor allocation, and the existing native
  one-million-control transfer limit also applies.
- This is a raw Gordon interpolant and can retain input and parameter-mapping
  kinks. General smoothing, best-fit handling of disconnected crossings, and
  surface-edge continuity matching belong to the unfinished NetworkSrf fitter.

The source-image certificate does not prove injectivity, topology, arbitrary
interior shape, symmetric boundary distance or Rhino fitting parity.

## Crossing-constrained curve refitting

The geometry crate provides `try_fit_cubic_curve_with_constraints`; the optional
native equivalent is `viboceros_smlib::fit_cubic_curve_with_constraints`. Both
reuse the existing arc-length sampler, adaptive fitter and fixed-row cubic
collocation. Each `CurveFitConstraint` supplies an original source parameter
and a target point. `ConstrainedCurveFit` returns the polynomial curve and the
corresponding arc-length parameters in the caller's original order.

Interior targets can move within the fit tolerance. Source endpoints and
retained kinks keep their natural positions and tangent handles. Constraints
introduce simple cubic knots, preserving C2 continuity away from retained
kinks. The solver replaces selected free collocation equations while retaining
the existing five-diagonal matrix layout. The native path uses usd-brep's
existing banded solver.

Every returned constraint has an exact rational stored-curve distance bound at
the supplied absolute numerical tolerance. An inaccurate or inconclusive
constraint rejects the result. The adaptive fit between constraints still uses
the existing sampled error policy; it is not a continuous source-curve distance
certificate. At most 64 distinct arc-length stations and 4,096 controls are
supported. Invalid source parameters, conflicting endpoint/kink targets and
targets outside the fit budget reject without changing the source.

This primitive fixes the missing central contact observed after independently
refitting the biquadratic network. The native regression fits six curves at
`1e-7`, detects all nine contacts and requires complete fitted-curve admission
on the resulting surface at `1e-6`. The
[validation record](constrained-network-fit-validation.json) covers both fitting
backends and the downstream Gordon construction. This does not qualify the
complete NetworkSrf fitter or its interior behavior.

The [common-parameter curve fitter](common-parameter-curve-fit.md) adds supplied
unit stations, a monotone scalar map and chain-rule endpoint/corner speeds.
Its reproducible network comparison retains both the improved arc-length
candidate and the worse global smooth-map candidate. Neither qualifies Rhino
interior fitting; the application command remains unregistered.

[Original-curve admission](original-network-curve-admission.md) now certifies
the six pre-fit originals for both refit candidates on the retained fixture.
[Exact isocurve restrictions](exact-isocurve-restriction.md) close the earlier
arc-length work-limit failure without enlarging proof limits.

## Rhino comparison and remaining work

The experimental `network_surface` oracle operation directly exercises this
native producer. Its reference uses the public auto-sorted
`NurbsSurface.CreateNetworkSurface` API with position continuity and the
explicit edge/interior tolerances in the request. All SDK inputs and outputs
are privately owned. Area is measured on an owned translated surface copy
because the SDK area integrator failed at a remote world origin; point queries
and retained NURBS definitions stay in the original world frame.

Nine fixtures cover planar and saddle patches, curved boundaries, biquadratic
and nonuniform networks, reversed directions, independent domains, rotation,
a remote origin and a rational cylinder. Raw observations and the
[comparison record](gordon-network-comparison.json) retain both successes and
mismatches. The requested comparison tolerances are `1e-6` for closest-point
output distances and `1e-5` for areas. These are diagnostic comparisons, not a
passed Rhino qualification. Independent Rust closest-point evaluation of the
saved Rhino tensors reproduces the distances on three representative queries.
The readback request is reproducible with
`python3 -m tools.rhino_oracle.network_surface_readback REQUEST OBSERVATIONS OUTPUT`.

Curved-network point-query outputs differ by roughly `0.028` model units in
the worst fixture, and areas differ as well. Full input-curve interpolation
therefore does not establish the requested Rhino interior behavior. Rhino's
output domains are consistent with average curve lengths; matching its
parameterization and fitting requires further work.
Crossing-constrained refitting is available. Next work is smooth parameter
alignment and fitting/interior comparisons before exposing the application
command. Closed networks, near-contact best fits, boundary surface
continuity, getter options and associative history remain part of the intended
NetworkSrf implementation.

Reproduce the candidate and reference captures with:

```sh
cargo run -p viboceros-oracle --features native-smlib -- \
  tools/rhino_oracle/fixtures/network_surface.json /tmp/gordon.json
python3 -m tools.rhino_oracle rhino \
  tools/rhino_oracle/fixtures/network_surface.json --output /tmp/rhino-network.json
```

The [construction validation record](gordon-network-validation.json) distinguishes
native/source-image checks from the failed Rhino interior comparison.

References: [Rhino NetworkSrf](https://docs.mcneel.com/rhino/8/help/en-us/commands/networksrf.htm),
[public network API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_NurbsSurface_CreateNetworkSurface.htm).
