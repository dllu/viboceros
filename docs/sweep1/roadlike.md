# Sweep1 Roadlike frames

[Command guide](../sweep1.md) · [Construction](construction.md)

`FrameStyle=Roadlike` uses a planar rail's support-plane normal as its fixed
axis, or world Z for a nonplanar rail. This follows the
[documented Rhino default](https://docs.mcneel.com/rhino/8/help/en-us/commands/sweep1.htm).
The existing OpenNURBS support-plane query supplies the normal, including its
line-plane convention. Inference uses the original rail before automatic
direction changes or fitting. `Axis=x,y,z` supplies an explicit override.
Parallel rail tangent and axis remain an error.

The command and JSON API share `SweepFrameStyle::roadlike_default`. JSON requests
select it with `"roadlike": true`; `"roadlike_axis": [x,y,z]` takes precedence.
Freeform remains the default frame style.

## Evidence

[Recorded validation](roadlike-validation.json) compares complete surface images
against eleven Rhino commands with explicit axes: XY, XZ, YZ, and oblique arcs
with retained and refitted rails, two lines including world Z, and a spatial
cubic. The line profiles make these surfaces affine in V. Exact rational
boundary-curve bounds with a shared monotone U correspondence therefore bound
the whole surface, including all faces and knot limits. Sources survive both
construction and undo. Explicit-axis rejection is tested separately.

Construction tolerance is `1e-7`. Retained-rail comparisons use epsilon `1e-7`;
refitted arcs use `1e-6`. The latter do not certify agreement within construction
tolerance; they retain the existing fitting limitation rather than changing the
fitter as part of axis inference.

The captured references distinguish three entry points. Explicit-axis commands
are the geometric reference. A separate SDK family applies the documented
default using Rhino's public `TryGetPlane` query. An older SDK diagnostic passed
`Vector3d.Unset`: its eight planar arc results are empty, while the two lines
and spatial rail return surfaces. These diagnostic results do not establish
default-axis compatibility. Original producer versions are archived with every
capture. The scripted `_Style=_Roadlike` axis getter accepts CPlane Z on Enter;
this prompt is separate from the documented automatic policy.

General multi-profile Roadlike compatibility, frame-style persistence, surface
edge alignment, and parallel-axis fallback remain unqualified. Existing
[Global blending limitations](validation.md) remain.
