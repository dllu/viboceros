# Contour

`Contour base-point direction-point spacing` intersects selected curves,
surfaces, B-reps and meshes with equally spaced planes. The full 3D picked
direction defines the plane normal; it is not projected into the CPlane. With
Range=No, planes extend in both directions from the base point across source
bounds, including when the base lies outside the objects. Sources stay intact.

Bare `Contour` accepts source selection, a base point, a direction point and
spacing. Spacing accepts a positive scalar, calculator expression with units,
or two snapped spacing points. Typed point coordinates use the shared CPlane,
world/relative syntax, filters and constraints. Esc cancels pending input without
geometry changes. Outputs and their groups form one Undo step.

Options:

- `Range=Yes|No` (default No). Range uses the picked interval: for length L and
  spacing D, native captures emit indices 0 through floor(L/D)-1. A range shorter
  than one spacing emits nothing. The far-end plane is excluded even when the
  spacing divides the range exactly. Type `Range` before the first point to
  enable this in the getter.
- `AssignProperties=ByCurrentLayer|ByInputObject` (default ByCurrentLayer).
  Current layer output gets fresh attributes; input output copies source
  attributes. Input group memberships are not copied.
- `GroupObjectsByContourPlane=Yes|No` (default No). Each nonempty plane gets one
  group. Captured group indices increase along the direction; output object
  records run in the opposite order.
- `Output=All|CurvesOnly`. Section styles and hatches are not implemented, so
  both currently emit curves and points.

Grid bounds and indices use exact rationals of binary64 coordinates. Each plane
origin is computed independently and rounded once; remote integer indices above
2^53 remain usable. Jobs exceeding 100,000 planes, collapsed representable plane
origins, nonpositive spacing and degenerate directions fail before output
admission. Standalone points and point clouds are ineligible sources. All cuts
stage before document mutation; the existing Section intersection-family limits
apply. Mesh cuts reuse exact dyadic plane/edge slicing and topology tracing.

Twenty-one native recipes cover both directions, off-grid and remote base
points, 3D normals, rotated CPlanes, whole/picked ranges, properties, groups,
meshes and surfaces. Sixteen match strict geometry, native parameter domain and
metadata checks at 1e-9 absolute + 1e-12 relative epsilon. Five surface recipes
match normalized samples and metadata but differ in parameter domains; they
remain strict compatibility failures. The report deliberately has `passed:false`.
Do not treat normalized samples as proof of parameterization equivalence.

An ineligible-source macro produced an invalid native curve with unset values.
Its raw result is retained separately, excluded from geometry evidence, and the
native helper now rejects all-ineligible inputs before launching a command.
Further surface/B-rep qualification, native domains/seams, hatches, remembered
options and performance remain open. See [comparison](../contour-command-comparison.json),
[provenance](../contour-command-provenance.json), and the
[ineligible-source diagnostic](../contour-ineligible-native-diagnostic.json).

Reference: [McNeel Contour](https://docs.mcneel.com/rhino/8/help/en-us/commands/contour.htm).
The recorded Rhino 8.32 behavior accepts full 3D normals even though the help
page also describes CPlane-perpendicular planes.
