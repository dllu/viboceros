# Surface area and signed volume

[Project overview](../README.md) · [Measurement commands](commands/editing.md)

`Area` measures analytic geometry, meshes, NURBS surfaces, and trimmed B-reps.
`Volume` measures closed meshes and consistently oriented, closed B-reps;
outward normals give positive volume and reversing every face gives negative
volume. Both commands preserve selection, geometry, attributes, and undo history.

[AreaCentroid](commands/area-centroid.md) integrates area first moments and creates
one cumulative point. Its independent exact weighted aggregation is separate from
document selection, grouping, and history. Mesh area and area moments split quads
along the shorter spatial diagonal (A-C on exact ties); display triangulation is
unchanged. [VolumeCentroid](commands/volume-centroid.md) adds exact mesh volume
moments and normalized B-rep flux integration. Mesh `Volume` now uses that same
shorter-diagonal policy. The command documentation retains the associated
Rhino numerical differences and independent high-precision reference checks.

`VolumeBoundary` integrates unjoined boundary pieces about one common reference.
`VolumeCentroid` asks before accepting open collections; its strict per-solid
kernel APIs are unchanged. See the [open-boundary audit](volume-centroid-open.md)
for the exact mesh reference expansion, curved flux densities, confirmation
semantics, and retained isolated-surface mismatch. The scalar `Volume` command
still requires closed input.

## Integration

B-rep measurements use the NURBS surfaces and parameter-space trim curves.
Display tessellation is not involved. Full domains and rectangular trims are
integrated per knot-span rectangle; planar trims use oriented boundary integrals.
Both planar and nonplanar boundary integrals are split at surface-knot crossings:
even a planar surface can have a non-affine UV map. This is necessary, for example,
after [subdividing a cylindrical cap's boundary](face-splitting.md).

For nonplanar trims, Green's theorem reduces the integral of a surface density
over the retained UV region to a boundary integral:

```text
∫∫ D f(u,v) du dv = ∮ boundary(D) [∫ u0..u f(s,v) ds] dv
area density:   |Su × Sv|
volume density: (S - reference) · (Su × Sv) / 3
```

Outer loops add area and clockwise inner loops subtract it. Face orientation
changes the volume sign. Inner quadrature is split at U knots. Outer quadrature
is split at trim knots and crossings of surface U/V knots, found from the rational
trim's Bernstein polynomials. This lets narrow surface spans and creases receive
their own integration intervals.

The implementation uses nested adaptive Gauss–Kronrod quadrature, compensated
summation, and centered surface controls. Derivatives are scaled to integration
intervals before cross products to reduce sensitivity to UV domain scaling.
Trim-curve parameters have a separate temporary frame: an origin is removed only
when every knot subtraction is exact, then exact power-of-two scaling keeps the
active range near unit size. Every scaling step must round-trip all knots,
including exterior knots. Control points, weights, multiplicities, relative span
widths, and stored domains are unchanged. Planar quadrature and nonplanar
surface-knot crossing isolation both use these frames; neither rounds integration
stations or roots back onto a coarse native parameter grid.

The absolute area budget is document distance tolerance times the control-bounds
diagonal; volume uses distance tolerance times its square. Budgets are divided
among faces, knot spans, boundary intervals, and inner integrations, alongside
relative error estimates. These are numerical estimates, not symbolic proofs.
Nonconvergence, nonfinite values, invalid trim domains, or exhausted work limits
return errors. Each boundary-integrated face allows at most 65,536 boundary intervals
and two million surface evaluations, in addition to quadrature subdivision limits.
An unrepresentable rescaling fails instead of collapsing knots. Lossless origin
removal can be declined when exterior knots prevent it, and arbitrary relative
span conditioning is not solved. These frames do not make every extreme-domain
curve admissible to the B-rep constructor.

## Validation

Analytic tests cover paraboloid disks and annuli, thin annuli, capped and reversed
solids, a rational cylindrical patch, translations of order `1e12`, changed UV
domains, and narrow knot spans in either surface direction. The capped graph
`z = x² + y²` below `z = r²` has volume `π r⁴ / 2`; its curved disk has area
`π ((1 + 4r²)^(3/2) - 1) / 6`.

The [Rhino oracle](oracle.md) constructs identical exact trim geometry in both
engines and compares area and signed volume. It requests `1e-11` absolute and
`1e-13` relative modelling/integration tolerances. Native fixture values are
independently tested against analytic formulas within `1e-12`. In the Rhino 8
probe, the rotated solid's volume differed from the analytic `π/32` by `1.21e-9`,
unchanged when the requested integration tolerances were tightened. The external
comparison therefore uses `1e-8` absolute tolerance, not a claim of `1e-11` agreement.
Fixture construction and command checks are outside the timed measurements:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/trimmed_mass_properties.json \
  --absolute-epsilon 1e-8 --relative-epsilon 1e-10
```

## Trim-domain regression audit

[35 parameter-frame fixtures](../tools/rhino_oracle/fixtures/trim_parameter_frames.json)
and [raw Rhino 8.32.26160.13001 observations](../tools/rhino_oracle/observations/trim_parameter_frames.json)
cover a planar disk, a paraboloid disk and annulus, and outward/inward capped
solids. Only trim-curve parameters change: origins are `0`, `±1e9`, and `±1e15`,
with additional domain lengths `4e-100` and `4e100`. Surface UV coordinates and
spatial edge geometry are unchanged.

All 35 live comparisons pass at the existing `1e-8` absolute / `1e-10` relative
oracle threshold, with maximum difference `1.212e-9`. Native values agree with
the analytic formulas within `4.5e-16` in this run; regression assertions use
`1e-12`. Rhino's unrounded integration residuals are retained, not replaced by
analytic values. The probe now records every trim's degree, controls, weights,
knots, and domain outside the timed section, and verifies Rhino measurement did
not change them. Both engines' trim records match the requested data exactly.
Native probes additionally exercise `Area`/`Volume` and check geometry, selection,
identity, and undo history remain unchanged.

Kernel regressions also cover surface-knot crossings, hole orientation, domain
lengths around `1e±280`, nonuniform cubic spans, exterior knots, and rejected
range loss. Frame-level point/derivative tests include the smallest subnormal
interval and endpoints at the finite binary64 limits.

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/trim_parameter_frames.json \
  --timeout 600 --absolute-epsilon 1e-8 --relative-epsilon 1e-10
```
