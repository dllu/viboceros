# Ribbon

[Curve commands](curves-index.md) · [Smooth geometry](../geometry/smooth-ribbons.md)

Run `Ribbon` with one curve selected, or pick a curve after starting the command.
The curve pick advances directly to the side prompt. Pick the desired side to
create a ruled ribbon. `Distance` accepts a positive number, a length with units,
or two picked points. Typed side points use the shared world/CPlane coordinates,
snaps and drafting controls. Escape cancels the draft without a model edit.

```text
Ribbon .5 0,5,0
Ribbon Distance=2cm 0,5,0 BothSides=Yes Corner=Round
Ribbon .5 0,5,0 Loose=Yes InCPlane=Yes OutputLayer=Input
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
Creation is one Undo step. Accepted distance, corner, Loose, InCPlane, layer and
tolerance settings belong to the registry; Undo does not reset them. BothSides
applies to the current invocation. Drafts invalidate when source state changes.

`Tolerance=0` or `Default` uses the document tolerance. A custom value controls
curve fitting; sewing still uses model tolerance. Loose ignores this fit
tolerance and Corner. ThroughPoint, Trim/self-intersection policies, source subcurves, history-based
regeneration and cursor previews remain unfinished. Both-sided Loose polycurves
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
