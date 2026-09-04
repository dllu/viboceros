# Surface area and signed volume

[Project overview](../README.md) · [Measurement commands](commands/editing.md)

`Area` measures analytic geometry, meshes, NURBS surfaces, and trimmed B-reps.
`Volume` measures closed meshes and consistently oriented, closed B-reps;
outward normals give positive volume and reversing every face gives negative
volume. Both commands preserve selection, geometry, attributes, and undo history.

## Integration

B-rep measurements use the NURBS surfaces and parameter-space trim curves.
Display tessellation is not involved. Full domains and rectangular trims are
integrated per knot-span rectangle; planar trims use oriented boundary integrals.

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
The absolute area budget is document distance tolerance times the control-bounds
diagonal; volume uses distance tolerance times its square. Budgets are divided
among faces, knot spans, boundary intervals, and inner integrations, alongside
relative error estimates. These are numerical estimates, not symbolic proofs.
Nonconvergence, nonfinite values, invalid trim domains, or exhausted work limits
return errors. Each nonplanar trimmed face allows at most 65,536 boundary intervals
and two million surface evaluations, in addition to quadrature subdivision limits.

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
