# NURBS support merging

The native `viboceros_smlib::merge_surface_supports` API provides the geometry
foundation for `MergeSrf`. Command selection, previews, attributes, Undo/Redo and
output B-rep edge construction remain in progress.

Supply two supports with agreeing boundary traversal. The first maximum U/V
boundary joins the second minimum U/V boundary in the requested direction.
The other parameter direction follows the first support's domain. A complete
curve certificate must bound their affine parameter correspondence within the
requested tolerance; unsupported certificates decline before the native call.
This includes boundary interiors, rather than checking only their endpoints.

SMLib's public `JoinSurface` operates on owned copies because compatibility
preparation can modify its operands. Zero knot-removal tolerance avoids an
approximate simplification. Exact knot insertion restores a redundant seam to
degree multiplicity for a sharp merge. Invalid scalars or directions leave both
inputs usable.

The homogeneous seam controls are averaged. Scaling just one operand's rational
weights can therefore change the merged shape, even though that operand's shape
stays the same. The public Rhino SDK exhibits this behavior too; captures retain
both large and small relative weight scales.

With `Smooth`, only the seam's two outer knots move toward their neighboring
distinct knots, by half the `Roundness` value. Control points stay fixed.
Existing interior knot groups retain their values. Smoothing deliberately
changes the surface; input edge tolerance does not bound that shape change.

Three headless Rhino SDK captures contain 30 authored cases: 28 successes and two
declines. They cover polynomial and rational supports, linear through quartic
degrees, unequal degrees, shifted UV domains, existing interior knots, direction
edits, small gaps and a bowed boundary with coincident endpoints. The
[finite validation record](merge-surfaces-validation.json) compares degrees,
control counts, knot clocks and 17 by 17 parameter grids within 1e-9 model units.
Native controls are expectations only. The failed initial Python import is
retained with its frozen producers.

These observations qualify the oriented support primitive. Broader closed or
periodic cases, nonlinear boundary-speed matching, output B-rep topology,
continuous surface-distance bounds and full command parity remain unqualified.
See the public [MergeSrf command](https://docs.mcneel.com/rhino/8/help/en-us/commands/mergesrf.htm)
and [MergeSurfaces API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_MergeSurfaces_1.htm).

```sh
cargo test -p viboceros-smlib --features native --test surface_merge
python3 -m tools.rhino_oracle.check_merge_surfaces
```
