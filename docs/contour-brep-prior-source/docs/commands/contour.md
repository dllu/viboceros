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

The original 21 native recipes now match strict geometry, parameter-domain and
metadata checks. The expanded suite matches all 52 captures (40 distinct recipes
and 12 repeats) at 1e-9 absolute + 1e-12 relative epsilon. It covers shifted and
negative UV ranges, weighted surfaces, 3D cutting planes and off-axis origins.
No domain fields are excluded from comparison.

Straight contours retain natural edge/isocurve domains or use signed distances
from the cutting-plane origin. Competing isocurves use the smaller-magnitude
fixed parameter; this is a rule inferred from independent native captures.
Positive-weight control-hull validation bounds the whole locus before converting
a higher-degree straight curve to an affine degree-one curve. Source geometry
is unchanged. See [parameterization](../contour-domain-parameterization.md).

An ineligible-source macro produced an invalid native curve with unset values.
Its raw result is retained separately, excluded from geometry evidence, and the
native helper now rejects all-ineligible inputs before launching a command.
Further surface/B-rep qualification, broader native domain/seam qualification, hatches, remembered
options and performance remain open. See [expanded comparison](../contour-domains-comparison.json),
[current provenance](../contour-domains-provenance.json), and the
[ineligible-source diagnostic](../contour-ineligible-native-diagnostic.json).

Reference: [McNeel Contour](https://docs.mcneel.com/rhino/8/help/en-us/commands/contour.htm).
The recorded Rhino 8.32 behavior accepts full 3D normals even though the help
page also describes CPlane-perpendicular planes.
