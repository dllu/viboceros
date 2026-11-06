# Experimental two-view reconstruction

`Crv2View` is **not registered**. Its curved outputs have not passed the existing
`1e-6` Rhino fidelity gate. The preparation API and immutable native evidence are
retained for further development; passing experimental tests does not qualify
the command. See the [validation record](curve-two-views-validation.json).

The geometry crate reconstructs shared monotone rational charts directly after
exact basis operations. Other profiles use intersections of ruled projectors,
with usd-brep through the native command backend. Common-sign control hulls bound
the projector extents in linear time; there is no arbitrary extrusion length.
Turning charts always use the intersector, since equal parameters alone can miss
additional spatial branches. Input, basis and output budgets bound the work.

The command crate stages immutable whole curves or temporary curve/edge ranges,
checks parents and document settings again at commit, and creates fresh unselected
curves in one transaction. Sources retain their geometry storage, attributes and
groups. No interactive controller is implemented.

## Captured behavior and remaining gaps

Five isolated Rhino captures retain 45 observations: 29 command workflows, five
grammar/SDK controls, five tolerance audits, three profile-fit audits and three
direct surface-intersection audits. Raw observations are gzip-compressed without
changing their bytes; content-addressed archives preserve the original producers.
Every capture ran on a private Xvfb display.

- Twenty-three non-cancel workflows replay through the local preparation API.
  Sources, selection, output attributes and Undo/Redo checks pass. Twenty-one
  construct outputs; disjoint and parallel profiles reproduce native failure.
- One successful line construction and those two failures meet the sampled gate.
  Nineteen workflows retain sampled distance upper bounds above `1e-6`, ranging
  from about `1.3e-5` to `1.05e-4`. An upper bound above the gate is inconclusive
  by itself. Equal circles also differ in output count: local two, native one.
- Six native cancellation captures remain unchanged-document controls only.
  They have no local controller replay and are excluded from local parity counts.
- Five independently known cubic audit cases have a stronger witness: exact
  rational Bernstein subdivision proves each stored native reference point lies
  more than `4e-5` from the **entire** known cubic. This is a narrow exclusion
  proof, not a certificate for the other workflows or a global error maximum.
- Finer SDK, document and angular tolerances do not remove that cubic discrepancy
  in these captures. Ordinary public curve fitting does not explain it. Direct
  public surface intersections show comparable profile discrepancies. These
  experiments do not establish a cause or reproduce Rhino's implementation.
- The parallel-direction SDK control returns a huge spurious curve. The command
  rejects parallel directions; the SDK anomaly is not a compatibility target.

Distance comparisons sample nine points per nonempty span in each direction.
The experimental comparator uses nearby stations to propose target parameters and
measures actual evaluated target points, so it reports sampled upper bounds.
Existing CurveBoolean comparisons retain their full closest-point search, with
reusable preparation exposed through `PreparedCurveClosest`.

## Reproduction

```sh
cargo test -p viboceros-geometry --lib curve_two_views
cargo test -p viboceros-command --lib curve_two_views
cargo test -p viboceros-oracle --test curve_two_views_audit -- --nocapture
cargo test -p viboceros-oracle --features native-smlib --test curve_two_views -- --nocapture
python3 -m unittest tools.rhino_oracle.test_curve_two_views
```

The frozen observations are comparison inputs, not regenerated expectations.
Future native captures need new family names. Geometry, topology, continuous
coverage, interactive behavior and comparative performance still need work before
registration.

Public references: [Crv2View command](https://docs.mcneel.com/rhino/8/help/en-us/commands/crv2view.htm)
and [Curve.CreateCurve2View](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/createcurve2view).
