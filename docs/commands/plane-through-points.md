# PlaneThroughPt

`PlaneThroughPt` fits one rectangular planar surface to selected point objects,
point clouds, or control/edit grips. Enable grips with `PointsOn` or `EditPtOn`
to use curve and surface controls or mesh vertices. Select sources first, or
start the command and pick sources before pressing Enter. `SelAll`, `SelPt` and
`InvertPt` work during grip picking. Escape cancels without editing geometry.

Every selected point occurrence contributes equally to the orthogonal
least-squares fit. A selected cloud contributes its complete point array.
Selected grips contribute Euclidean positions, independently of NURBS weights.
Sources, attributes, groups and grip display are preserved. The output uses
default attributes on the current layer and starts unselected. Preselection is
retained; objects picked after starting the command are deselected on acceptance.
A successful creation is one Undo/Redo edit.

```text
Point 0,0,2
Point 4,0,2
Point 4,3,2
Point 0,3,2
SelPt
PlaneThroughPt
```

The rectangle encloses the points' projections in the fitted plane's frame;
it does not seek a minimum-area rectangle. Its degree is 1 in each direction,
with two controls per direction and parameter domains equal to physical edge
lengths. Fewer than three input points rejects. Coincident inputs and rectangles
with a collapsed dimension produce no surface. Collinear inputs use a
deterministic containing plane and can produce a rectangle with nonzero area.
The existing 1,000,000-point fitting limit applies.

The geometry implementation reuses the existing exact-rank, exactly centered
faer least-squares fitter, the shared normal-derived frame, and the existing
bilinear tensor constructor. usd-brep's `N_FitPlaneToPts` instead chooses a
minimum-area bounding rectangle; it does not supply this command's measured
rectangle policy. No second plane solver is introduced.

[LineThroughPt](line-through-points.md) shares the centered SVD, source
gathering and selection/history machinery. Each command retains its own input
filter, minimum point count and no-result policy.

Seventeen native commands captured on private Xvfb cover point preselection
and postselection, clouds, mesh and curve grips, all coordinate planes, rotated
rectangles, triangles, tilted/nonplanar inputs, a remote origin and degenerate
sets. Command and oracle regressions match one of the eight rectangular tensor
chart symmetries and use exact point-distance admission at all four controls.
Convexity then bounds the complete bilinear image within `1e-6`. They also check
source lifetimes, selection and output attributes. Eigenvector
signs can reverse surface normals or UV charts; oriented tensor parity and
ambiguous repeated-eigenvalue fits remain unqualified. Local tests additionally
cover Undo/Redo, cancellation, accumulated viewport grip picks, fitting limits,
and small/large finite coordinate scales.

The largest paired control distance is `9.60e-10`. Four raw tensor comparisons
still differ in orientation or chart direction; the unchanged definitions and
all geometric certificates are retained in the
[comparison record](../../tools/rhino_oracle/observations/plane_through_points_comparison_20261016.json).

The Python/Rust oracle operation is `plane_through_points`, with `points`,
`source` (`points`, `cloud`, `mesh_grips`, `curve_grips`) and `postselect` fields.
It returns complete raw surface definitions, corners, source selection and
output attributes. Raw protocol comparison keeps normal/UV differences visible;
the complete image comparison above admits an explicitly matched chart.

```sh
python3 -m tools.rhino_oracle viboceros \
  tools/rhino_oracle/fixtures/plane_through_points.json --timeout 600
```

See [McNeel's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/planethroughpt.htm),
the [retained recipes](../../tools/rhino_oracle/fixtures/plane_through_points.json),
the [native captures](../../tools/rhino_oracle/observations/plane_through_points_rhino_20261016.json),
and the [validation record](../plane-through-points-validation.json).
