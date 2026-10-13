# BlendSrf

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/blendsrf.htm)

Run `BlendSrf`, pick a complete edge on each of two single-face surfaces, and
adjust the displayed preview. Enter accepts it; Escape cancels without changing
the document. Ambiguous edge picks list numbered choices in the command log.
Options can be changed with `Continuity1=G2`, `Bulge2=1.5`, or `Join Yes`.
Preview diagnostics report position error, tangent-plane angle and scaled
first/second derivative residuals. Each accepted operation is one undo step.

For scripts, select two surface objects or specify `Sources=id,id`. `Edge1` and
`Edge2` use natural surface sides: West = minimum U, East = maximum U, South =
minimum V, North = maximum V. Defaults are East and West. To use actual B-rep
edge table indices, provide `BlendSrf object-id edge-index object-id edge-index`.
Indices start at zero and must identify complete naked edges. `Preview=Yes`
performs readonly construction and diagnostics without adding geometry.

```
BlendSrf Edge1=North Edge2=South Continuity1=G2 Continuity2=G2
BlendSrf Sources=1,2 Bulge1=0.6 Bulge2=1.8 Reverse2=Yes
BlendSrf 1 1 2 3 Trim1=0.2 Trim2=0.1 Join=Yes
```

`Continuity1` and `Continuity2` independently accept `G0`/`Position`,
`G1`/`Tangency`, or `G2`/`Curvature`; both default to G1. The command matches
the complete rational boundary and its requested derivatives using homogeneous
Hermite interpolation. G0/G0 produces a ruled patch; G1/G1 is cubic; G2/G2
is quintic. Different parent degrees, knots, parameter ranges, sizes and positive
rational weights are supported without refitting the boundaries. G1/G2 require
regular tangent planes at the checked boundary locations.

`Reverse2=Auto` (default) chooses the smaller summed distance between five
corresponding edge samples. `Yes` or `No` explicitly controls the second edge's
correspondence. Closed collars retain their existing seams; move them with
`SrfSeam` before blending when the seams need alignment.

`Bulge1` and `Bulge2` are positive independent shape factors, defaulting to one.
The corresponding derivative multiplier is half the parent's cross-boundary
parameter extent times the bulge. Thus two cylindrical collars whose physical
lengths are L1 and L2 can reproduce a straight axial blend parameterization
over a gap D using `Bulge1=2*D/L1` and `Bulge2=2*D/L2` (calculate the numbers
before entering them). Extreme bulges that create nonpositive rational weights
are rejected; reduce the offending factor.

`Trim1` and `Trim2` remove a fraction of their parent's width measured inward
from the selected edge, then blend from the new boundary. Values range from zero
through less than one. Without `Join`, both parent identities and attributes
are retained, trimmed parents are updated, and a separate blend is added on
the current layer. `Join=Yes` must join both interfaces; otherwise the operation
fails without editing the document. A successful join retains the first parent's
object ID, attributes and group memberships and removes the second object.

The current workflow supports single-face surfaces with rectangular trims,
including complete closed elliptical collar boundaries. Use `ExtractSrf` to
detach a face from a polysurface. Arbitrary curved UV trims, edge chains,
partial boundary segments, draggable shape handles, G3/G4 and history-linked
editing are pending. Boundary continuity and the sampled diagnostics do not
certify global fairness or absence of folds and self intersections. This is an
independent construction; Rhino's default blend shape is not reproduced.

Regression coverage includes all natural edge orientations, mixed continuity,
unequal rational spline bases/domains, trimming, joining, cancellation, stale
previews, undo/redo and OpenNURBS 3DM readback. The supplied ASTRA V3 collar
fixture also checks the independent minimum-jerk transition across a 41×41 grid.
