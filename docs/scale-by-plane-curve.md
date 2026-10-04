# Curve representation and Object plane evidence

[ScaleByPlane](scale-by-plane.md) · [CPlane](cplane.md) · [Capture provenance](scale-by-plane-curve-provenance.json)

The retained `scale_by_plane_curve.json` request and observations contain 64
public Rhino recipes: 16 curve representations, two supporting planes, and
forward/reverse orientations. Each runs in an empty owned document at idle on
private Xvfb. Rhino version: 8.32.26160.13001.

| Representations | Native frame observation |
| --- | --- |
| Analytic circle, ordinary arc, angularly offset arc | Supporting circle center and original plane axes |
| Exact NURBS circle and arcs | Center, radial direction toward the physical start, and start tangent |
| Unequal-radius ellipse and its NURBS form | Start point and start tangent |
| Single-leaf PolyCurves | Corresponding leaf frame |
| Mixed line/NURBS/arc and nonplanar PolyCurves | Curve start and tangent with an oriented supporting frame |

An analytic offset arc has angular interval `[0.35, 2.2]` independently of its
supporting axes. Reversal keeps plane X, flips Y/Z, and changes its angles to
`[-2.2, -0.35]`. Rebasing the supporting plane to the geometric start loses this
state and produces a different Object scale. The captures distinguish those
outputs from equivalent NURBS arcs. Circular NURBS recognition in the kernel
uses its existing certificate across every Bezier span.

The kernel retains supporting-plane X and the angular interval in `CircularArc3`,
while keeping a cached start frame for evaluation. Trim, extension, reversal,
seam changes, and similarity transforms preserve the relation between these
representations. The OpenNURBS bridge retains the supporting frame, angles, and
independent curve domain for standalone arcs and PolyCurve leaves. Its private
codec also reads the preceding arc tag.

## Verification

The probe obtains an independent public `CPlane Object` witness before running
`ScaleByPlane`. Python independently projects four source points onto those
axes and compares native outputs at `1e-9`. It checks public EndCommand success,
post-macro state, selection, names, and external Undo/Redo.

Application tests replay all 64 recipes as complete commands and incremental
picks, for 128 workflows. They compare CPlane origin/axes, scaling, target
invariance, metadata, selection, and geometry history. An additional 20 point
CPlane replays use witnesses from the preceding
[Object matrix](scale-by-plane-object.md): a point moves the origin while
retaining the active CPlane axes.

Local CPlane Undo/Redo checks in these tests are not independent native history
captures. An actual OpenNURBS `.3dm` round trip checks 24 analytic arc/circle
objects and single-leaf composites against the recorded SDK recipes, including
supporting axes, angles, domains, classes, and sampled geometry. That file test
is a bridge check, not an additional Rhino reader capture.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_by_plane_curve.json --scheme VibocerosOracleCurvePlane --output tools/rhino_oracle/observations/scale_by_plane_curve.json --timeout 360
python3 -m unittest tools.rhino_oracle.test_scale_by_plane_curve
cargo test --release --bin viboceros scale_by_plane_curve
cargo test --release -p viboceros-io recorded_rhino_arc_planes --lib
```

Raw observations and capture helpers have SHA-256 provenance. A replacement
capture requires updated provenance and review. Copy/Rigid combinations for
these curve families, general B-rep face picking, extreme numeric ranges,
previews, speed, and full native parity remain unverified.
