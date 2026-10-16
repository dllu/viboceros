# Sphere FitPoints

`Sphere FitPoints` fits a sphere to at least three point objects or selected
control/edit grips. Select sources first, or enter `Sphere`, choose `FitPoints`,
pick sources and press Enter. `PointsOn`, `SelAll`, `SelPt` and `InvertPt` share
the existing grip workflow. Escape cancels the selection.

```text
Point 1,1,1
Point 1,-1,-1
Point -1,1,-1
Point -1,-1,1
SelPt
Sphere FitPoints
```

Whole point clouds and meshes are excluded by this command's selection filter.
Mesh vertex grips are accepted. Each selected point occurrence has equal
weight, including duplicates; NURBS grips contribute Euclidean positions.
Coincident and collinear selections decline without creating geometry.
Three noncollinear points and planar inputs choose a center in their plane.

The output is the existing exact 9-by-5 rational quadratic sphere construction
on the current layer with default attributes. It starts unselected. Sources
and their attributes remain intact. Preselected sources stay selected; objects
picked during the command are deselected on acceptance. Creation is one
Undo/Redo edit. Cancelled or declined fits preserve Redo.

The kernel normalizes coordinates, uses faer thin SVD for rank and an algebraic
seed, then refines equal-weight radial residuals. The radius is the mean point
distance at each candidate center. Planar inputs reuse the circle fitter;
circles and spatial spheres share the same bounded damped radial refinement.
There is no second nonlinear solver or new native dependency. The existing
one-million-point fitting limit applies. Unresolved numerical failures reject;
global optimization and performance relative to Rhino remain unqualified.

The [21-case fixture](../../tools/rhino_oracle/fixtures/sphere_fit_points.json)
covers preselection, postselection, points, clouds, meshes, grips, duplicate
weights, reversed input order, planar and spatial noise, remote coordinates,
scaling, degeneracy and a shallow spherical cap. The native command creates
16 outputs; 15 agree within `1e-6` over their complete rational surface images.
The public SDK produces 18 fits; 17 agree within `1e-6` using the bound
`distance(center, reference_center) + abs(radius - reference_radius)`.

The retained exception is the shallow cap. Its 20 inputs lie on the sphere
centered at `(0,0,0)` with radius `2` within `1e-12`. Our fit recovers that
sphere. Both the captured Rhino command and SDK instead return a sphere near
`(0.000593,-0.000064,1.886458)` with radius `0.117204`; some source points miss
that sphere by more than `1e-6`. The parameter/image bound differs by about
`3.77`. This comparison remains failing. General Rhino Sphere FitPoints parity
is not qualified.

The `sphere_fit_points` Python/Rust oracle operation retains the original
point array, source type and `postselect` flag, complete output NURBS
definitions, attributes, source selection and separate SDK observations.
The native probe appends Enter and Cancel to finish or cancel incomplete
getters, including unsupported whole-cloud/mesh inputs and two-point inputs.
It runs in a private oracle scheme. The raw reports preserve the cap discrepancy.

See the [native capture](../../tools/rhino_oracle/observations/sphere_fit_points_rhino_20261016.json),
[local response](../../tools/rhino_oracle/observations/sphere_fit_points_viboceros_20261016.json),
[comparison](../../tools/rhino_oracle/observations/sphere_fit_points_comparison_20261016.json)
and [validation record](../sphere-fit-points-validation.json).
