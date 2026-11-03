# Curvature analysis

[Analysis commands](analysis-index.md) · [Draft angle analysis](draft-angle-analysis.md)

`CurvatureAnalysis` displays persistent false colors on surfaces, polysurfaces
and SubDs. Select inputs first or pick them when prompted. Preselection remains
selected; command-first picks are released. `CurvatureAnalysisOff` closes the
panel and clears curvature-analysis membership.

The panel offers Gaussian curvature, absolute mean curvature, minimum radius
and maximum radius. Blue and red bounds can run in either order. Explicit equal
bounds leave the previous interval intact, matching Rhino's settings API.
Gaussian values have inverse-square length units, mean values inverse length,
and radii model length units. A saddle's maximum radius is infinite because it
has a direction of zero normal curvature.

Colors stay opaque and unlit in Wireframe, Shaded and Ghosted. Analysis faces
remain pickable in Wireframe. Edge/isocurve display is optional. Gray indicates
unavailable curvature. Add/Remove Objects affect membership; selection changes
alone do not. Hide/layer visibility suppress drawing, Delete/Undo remove and
restore the live display, and settings stay outside document history.

Curvature and draft-angle colors are mutually exclusive per object. Starting
either replaces the other's membership only for its targets, leaving other
objects and independent direction/curvature graphs alone. Display controls and
Auto Range and Max Range can be used during an active point getter.

Console extensions expose panel controls:

```text
CurvatureAnalysis [Add|Remove|MaxRange|AutoRange]
CurvatureAnalysis [Style=Gaussian|Mean|MinRadius|MaxRadius]
                  [Range=blue,red] [ShowIsoCurves=Yes|No]
CurvatureAnalysisOff
```

Differential samples share immutable source snapshots across viewports. Surface
parameters and crease sides come directly from the tessellator; no closest-point
search is needed while drawing. Colors cache independently of cameras. Affine
previews transform second derivatives, so nonuniform scale/shear affect curvature.
Regular SubD samples use public OpenNURBS bicubic limit patches, respecting crease
sectors. Samples without a regular patch remain unavailable, including
extraordinary corners. Smooth NURBS pole
limits use a converged incident parameter ray; this is not a general proof of
path independence. Source geometry and mesh colors remain unchanged.

52 frozen Rhino 8 recipes qualify model state, selection, analysis ownership and
manual ranges. Regular principal curvatures agree at `1e-7 * (1 + |k|)` (including
native umbilic splitting noise). All 11,536 saved surface color samples agree
within one 8-bit channel step; native mesh and separate SDK evaluations can fall
on opposite sides of a rounding threshold. SubD qualification covers command
state plus regular-patch/flat-surface checks, not general native curvature parity.

Auto Range clips outliers using the mean absolute deviation, keeps narrow
Gaussian intervals on their measured side of zero, symmetrizes broader ones,
and limits positive spans when the lower bound is at least 0.001 in that
metric's units.
It leaves settings unchanged when every member is hidden. Both range controls
use distinct vertex/crease-side samples, exclude values at or above `1e20`
(Gaussian) or `1e10` (other styles), and reset singleton/undefined intervals to
style defaults. Each style retains its own interval.

536 synthetic SDK cases isolate statistics from tessellation. Ten record native
rounding artifacts: certain constant multi-mesh inputs widen unexpectedly, as
does one interval differing by a single floating-point step. Viboceros retains
constant intervals and pads narrow positive intervals consistently. The frozen
native outputs preserve these differences. Another 32 Auto Range and 12 Max
Range panel captures document real-surface behavior. Bounds from the two
products' analysis meshes can differ; these captures do not qualify identical
tessellation or pixels. [ExtractAnalysisMesh](extract-analysis-mesh.md) copies the
current analysis representation and colors into model mesh objects. Shared
[analysis mesh controls](analysis-mesh.md) set NURBS density and SubD subdivision.
Ordinary meshes without surface curvature data are unsupported.

```sh
cargo test -p viboceros-oracle --test curvature_analysis_native
cargo test -p viboceros-oracle --test curvature_auto_native
cargo test -p viboceros-command --lib curvature_analysis
cargo test -p viboceros-geometry --lib tessellation::curvature
cargo test -p viboceros --bin viboceros curvature_analysis
cargo test -p viboceros --bin viboceros curvature_colors_are_unlit -- --ignored
python3 -m unittest tools.rhino_oracle.test_curvature_analysis
python3 -m unittest tools.rhino_oracle.test_curvature_auto
```

Reference: [Rhino CurvatureAnalysis](https://docs.mcneel.com/rhino/8/help/en-us/commands/curvature.htm).
