# Ribbon

[Curve commands](curves-index.md) · [Smooth geometry](../geometry/smooth-ribbons.md)

Run `Ribbon` with one curve selected, or pick a curve after starting the command.
The curve pick advances directly to the side prompt. Move the cursor to either
side to preview the ruled ribbon, then click or type a point to create it.
Edit Ribbon options before selecting the curve or while picking the offset side.
`Distance` accepts a positive number, a length with units, or two points.
After the first distance point, enter a length to constrain the second pick.
Choose `ThroughPoint` to vary the width with the cursor and pick or type the
target location. Both modes use the shared coordinate and snapping controls.
Escape cancels the draft without a model edit.

At a corner, the larger perpendicular distance to either adjacent segment
determines the side and the ThroughPoint width. The two segments at a closed
curve's seam participate in the same rule. At zero perpendicular distance,
a fixed-distance pick uses the curve's positive offset side. ThroughPoint
requires a nonzero width. [Rhino 8 side picks](../ribbon-side-reference.json)

Before selecting the source, enter `SubCrv` to use a temporary portion.
Select the curve and pick or type its start and end. You can also enter a
length and confirm its direction. The Ribbon side prompt then previews the
portion. A temporary source uses the current layer even with
`OutputLayer=Input`. [SubCrv controls](subcurve.md)
[Rhino 8 temporary-source capture](../ribbon-subcurve-reference.json)

```text
Ribbon .5 0,5,0
Ribbon Distance=2cm 0,5,0 BothSides=Yes Corner=Round
Ribbon .5 0,5,0 Loose=Yes InCPlane=Yes OutputLayer=Input
Ribbon ThroughPoint=w2,3,0
Ribbon 1 3,2,0 Trim=No
```

Ordinary ribbons support lines, planar smooth NURBS and planar polylines, including
Sharp, Round, Smooth, Chamfer and None corners. Regular curved `InCPlane` offsets
and nonplanar inputs also support fixed-normal cubic fitting. Analytic curves are converted to
NURBS without modifying their source objects. Loose ribbons use the original
Greville positions and a polynomial offset. `InCPlane=Yes` keeps those positions'
heights while offsetting perpendicular to the construction-plane normal.
Nonplanar Loose input uses that same construction-plane policy automatically.
Lines require a nondegenerate projection into the active construction plane.
Ordinary fixed-plane ribbons preserve spatial end jets while refining the original
knot spans; linear projections translate the original rational curve. Nearly
parallel/perpendicular control planes have unresolved native mode switches and
are conservatively rejected, except parallel planar inputs and linear projections.
Custom coarse fixed-plane fits currently support polynomial single-span sources;
rational and multispan coarse fits remain unsupported.

`OutputLayer=Current` creates fresh default attributes. `Input` copies the source
attributes, including its name and color overrides, onto its original layer.
Outputs do not inherit source groups. Preselected inputs remain selected;
command-first inputs and all results are unselected after completion and replay.
Creation is one Undo step. Distance, Corner, Loose, Trim, InCPlane, OutputLayer
and Tolerance edits carry over after Escape and Undo. BothSides and ThroughPoint
apply to the current invocation. A successful ThroughPoint pick remembers the
inferred width. The [Rhino capture](../ribbon-through-point-reference.json)
includes line extensions, off-plane picks, and rounded polyline corners.
Drafts invalidate when source state changes.

`Trim=Yes` clips adjacent inside corners of ordinary polyline ribbons.
`Trim=No` preserves the full ruled strips where they overlap. The live preview
follows the setting. [Rhino 8 trim capture](../ribbon-trim-reference.json)

`Tolerance=0` or `Default` uses the document tolerance. A custom value controls
curve fitting; sewing still uses model tolerance. Loose ignores this fit
tolerance, Corner, and Trim. Global self-intersection trimming and history-based
regeneration remain unfinished. Both-sided Loose polycurves
are rejected because their native face incidence is not yet matched. Constant
offset distance between fitting stations, winding and performance parity remain
unqualified. These limits do not imply full Rhino command parity.

The [fixed-plane validation record](../ribbon-cplane-validation.json) extends the
[original command record](../ribbon-command-validation.json) with complete ruled
image, incidence, attribute, selection and Undo/Redo comparisons on both backends.
It retains 85 complete matches and 46 atomic unsupported cases per backend.
Every captured native success is accounted for, including unresolved plane switches.
The strict geometry gate remains `1e-9`; independent rational boundary gauges do
not imply matching U parameters. Rhino captures run in private Xvfb sessions;
headless application tests exercise the getters.
