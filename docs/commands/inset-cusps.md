# Tight planar Inset cusp loops

[Curved boundaries](inset-curves.md) · [Output modes](inset-outputs.md)

With `Loose=Off`, a smooth closed boundary can offset beyond its first
curvature cusp while leaving a valid inset region. The normal locus reverses
locally and develops small loops. Inset removes oppositely wound loops, joins
the surviving arcs, and validates the complete region before staging output.
The original seam remains when it survives trimming. New cap kinks use the
existing one-degree edge-splitting threshold. Curved G2 edge partitions keep an
annular border for either `CreaseCorners` setting; see [partition controls and
remaining creased cusp fans](inset-partitions.md).

The shared Hermite fitter represents the full signed normal locus with a tighter
fit budget. A bounded Bézier-hull sweep finds distinct forward visits; derivative
control hulls exclude reversed stretches. Signed polynomial areas and cyclic
interval containment select the smallest reversed loops, including cases where
both complementary arcs contain negative area. Structural intersection and trim
operations use usd-brep when enabled. Complete collapse, ambiguous contacts,
region splitting, and creased cusp fans still return errors.
Support-strip bounds reject provable collapses before the contact search.
Exact separating-plane predicates also certify adjacent Bézier spans at sharp
joins without a tangent-intersection search.

Frozen private-display evidence contains 24 SDK cases and 72 real commands:
ellipses at distances 2.6–4.0, cubic Bézier and periodic spline boundaries, a
tilted ellipse and a degree-seven representation. Comparisons cover 22 valid
SDK results, two null SDK collapses, 66 successful commands and six command
failures. Command checks include all three output modes, metadata, selection,
undo/redo and 3DM roundtrips. Failed commands retain their source.

At `1e-6` model tolerance, independent fitted curves allow `2e-6`. The near-collapse
6-by-4 ellipse at distance 3.99 uses `1.5e-5`: Rhino's fitted intersection lies
about `1.2624e-5` from the analytic tip. A frozen regression measures this native
error. Independent kernel tests check the exact ellipse intersection formula at
2.67, 3.0 and 3.99 to `1e-7`; the kernel retains that more accurate locus.
These spatial comparisons sample both directions and do not prove a continuous
error bound.

```sh
cargo test -p viboceros-geometry --lib cusp_
cargo test -p viboceros-oracle --test inset_surface cusp_
cargo test -p viboceros-oracle --features native-smlib --test inset_surface cusp_
python3 -m unittest tools.rhino_oracle.test_inset_cusps
```
