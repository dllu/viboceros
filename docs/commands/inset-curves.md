# Curved planar Inset boundaries

[Surface Inset](inset-surfaces.md) · [Meshes and SubDs](inset.md)

Planar `Inset` supports smooth closed NURBS loops, circles and circular annuli,
including narrow annuli and multiple holes. Polygon loops and smooth curved loops
can occur on the same face. Tangent-continuous line/arc chains also work.
The supporting surface and original boundaries remain unchanged.

With `Loose=Off`, `CreaseCorners=No` keeps a smooth offset loop as one closed edge with an annular
border. `Yes` splits a chain at its original edge joins and adds connecting strips;
a single smooth closed source edge still has an annular border. The same command,
face picker, preview, cancellation, attributes and history rules apply to all inputs.
Previews sample every nonempty curve span and share the existing segment limit.

Certified circular/linear recognition retains analytic offsets and source clocks
where possible. Other smooth curves share the planar NURBS offset fitter. Builds
with `native-smlib` supply usd-brep's existing offset, intersection and curve-edit
operations; admission and topology policy remain shared. Endpoint correspondence,
closed-loop simplicity, winding, pair contacts and nesting are checked before
staging the complete result. Narrow holes use trim-region queries rather than
requiring their control hulls to fit inside an inscribed polygon.
Convex-loop certificates use an exact convex hull; hole-pair candidates use a
cached-bound sweep to avoid scanning unrelated boundaries.

Tight offsets of a single smooth closed edge can remove cusp loops while
retaining one cap boundary. The original seam can disappear, and new tangent
kinks split cap edges. See [cusp trimming and numerical limits](inset-cusps.md).

Frozen comparisons cover ellipses, periodic convex/concave splines, cubic Bézier
chains, stadium outlines, mixed polygon/circular loops, several holes, tilted
circles and narrow annuli. Analytic loci use `1e-10`. General fitted SDK curves use
`1e-6`; joined command curves allow `2e-6` for two independent fits at `1e-6` model
tolerance. The recorded ellipse command has about `9.5e-7` maximum sampled
source-distance error, while the shared fit has about `1.8e-8`. A separate regression
measures these against the original curve. Corresponding-edge comparisons run in
both directions at each curve's span stations; they do not require identical knot
tables. These remain sampled numerical fit checks, not continuous error proofs.

`Loose=On` moves original Greville edit points along planar offset normals,
retaining the original degree and rational weights. Periodic curves interpolate
cyclically before clamping. It is an approximation rather than a fit to model
tolerance. New internal tangent kinks exceeding one degree split cap edges while
retaining the stored seam. A single source edge still has an annular border,
even when its Loose cap is split into several edges. The split threshold is
independent of the supplied angular tolerance, as recorded distance and degree
controls demonstrate. Loose starts at Off each invocation; cancellation discards
pending distance and crease settings.

The original-basis solve reuses edit-point interpolation, with usd-brep supplying
the native backend when enabled. Portable interpolation uses banded systems and
a small cyclic border, with O(n*p) storage and O(n*p² + p³) work for degree p.
General Loose offsets currently admit degree 17 and 1000 stored controls, with
same-sign rational weights and exact periodic tails. Analytic shortcuts retain
their existing admission. Thirty frozen coefficient comparisons include periodic
convex/concave splines and degree-elevated ellipses; they use `1e-10` positions and
`1e-12` weights. Eighteen additional joined Loose commands cover geometry,
metadata, selection, 3DM roundtrips and real undo/redo. Seven native preference
steps distinguish completed settings from canceled changes.

The first legacy command capture includes a rejected `Loose=No` token inside the
getter. Later captures use the native `Loose=On|Off` spelling, with no unknown
tokens. All successful captures retain their exact producers and are immutable.

Curved G0 corners, higher-degree polygon-edge encodings, non-affine surface charts,
seams, poles and region-splitting offsets remain pending. Unresolved cusps, touching
or crossing boundaries, nested holes and exhausted fitting/storage budgets return
errors without modifying the document. `ThroughPoint`, seam options and
region-splitting tight offsets remain unimplemented. Curve and separate-surface
output are described in [output modes](inset-outputs.md).

```sh
cargo test -p viboceros-oracle --test inset_surface --test inset_loose
cargo test -p viboceros-oracle --features native-smlib --test inset_surface --test inset_loose
python3 -m unittest tools.rhino_oracle.test_inset_planar_curves tools.rhino_oracle.test_inset_loose
```
