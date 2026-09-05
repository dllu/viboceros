# Transforms and arrays

[Command reference](README.md) · [Project overview](../../README.md)

## Interactive transforms

`Polygon`
defaults to four sides, or accepts a side count such as `Polygon 6`. With
objects selected, enter `Move` or `Copy`
to pick a base and destination point, `Scale`, `Scale1D`, `Scale2D`, or

`Rotate` to pick center/reference/target points, `Mirror` to pick a two-point
axis, or `ArrayLinear 4` to pick its two spacing references. `ScaleNU` accepts
independent world x/y/z factors; all scale variants accept `Copy=Yes`.

`Rotate3D` picks an axis start/end followed by angle reference/target points;

`Rotate`, `Rotate3D`, and `Mirror` also accept `Copy=Yes`.

`Shear` picks a fixed origin, reference direction, and target angle in the top
view; its third argument can instead be a numeric angle, and it accepts
`Copy=Yes`.

`ProjectToCPlane` flattens onto the current construction plane (World XY in the
current UI). It retains the inputs by default; use `DeleteInput=Yes` to project
them in place.

## Orientation and arrays

`Orient` maps two reference points to two target points with Rhino's shortest
3D rotation. `Scale=No` preserves size, `Scale=1D` changes only the reference
axis, and `Scale=3D` scales uniformly. `Orient3Pt` maps right-handed frames
defined by three reference and target points; its `Scale=Yes` factor comes from
the first point pair. Both commands default to in-place, unscaled transforms;
`Copy=Yes` preserves the original selection, attributes, and group topology.

`OrientOnSrf` uses a source base/reference pair and a point nearest the target
NURBS surface. Name that surface with `SurfaceName=`, or make it the last
selected object. `Rigid=Yes` (the default) maps a frame without deformation;
`Rigid=No` applies the plane-to-surface point map; all supported curves,
including lines and polylines, receive tolerance-driven adaptive cubic fits,
untrimmed NURBS surfaces use checked control mapping or adaptive bicubic fitting,
trimmed B-reps fit shared edges and underlying faces while retaining exact UV
trims, and meshes map per vertex.
`Scale=` must be positive,
`Rotation=` is in degrees, `Flip=Yes` reverses surface Y and Z, and
`SourceNormal=` defaults to world Z. In deformable mode, `ConstrainNormal=Yes`
keeps normal offsets parallel to `SourceNormal`, the command-line stand-in for
Rhino's placement-viewport construction-plane normal.
This command defaults to `Copy=Yes`; originals remain selected and copied group
topology is preserved. Use `Copy=No` for an identity-preserving in-place morph.

See [curve morphing](../curve-morphing.md), [surface morphing](../surface-morphing.md),
and [B-rep morphing](../brep-morphing.md)
for native parameter and knot-limit
preservation, fitting limits, and separate direct-map versus fitted-output
comparisons. The earlier fixed-cubic line fitting has been replaced. Rhino's
tested fitted curves differ from its direct point map by more than the requested
document tolerance, so their comparison epsilon is documented separately.

`Array` takes X/Y/Z counts followed by signed world-axis distances. Its default
`UnitCell` mode uses those values as successive spacing. `Mode=Fill` treats them
as outside dimensions and accounts for the selected geometry's bounding-box
extent, matching Rhino's fit-within-span behavior. Counts of one create no
copies on that axis.

`ArrayLinear` takes Rhino's total item count followed by two reference points;
their vector is the spacing between successive items. It retains the originals
as the selection, preserves object attributes, and recreates every selected
group topology independently for each copy as one bounded, atomic undo step.

`ArrayCrv` places the selected sources at equal arc-length positions on a line,
analytic curve, polyline, or NURBS rail. Name a unique rail with the single-token
`PathName=` option, or omit it and make the rail the last selected object.
An item count includes both endpoints of an open rail and omits a duplicate
closed seam; `Distance=` uses a fixed spacing and leaves any shorter remainder.
`Freeform` is the default rotation-minimizing orientation. `Roadlike` keeps its
Y axis parallel to the world XY construction plane, `Stairlike` applies yaw
only, and `NoRotation` translates without rotating. `BasePoint=` uses an
explicit source anchor and, as in Rhino, retains the originals in addition to
all requested rail items. Without it, the originals are the first item. Use

`Flip` on the rail to reverse its travel direction. Freeform arrays use shared
[adaptive curve frames](../curve-frames.md) and incoming tangents at exact
corners. Rhino's query-dependent twist at some sharp spatial corners remains
an explicitly diagnosed compatibility gap.

`ArraySrf` copies every selected source into a U-by-V grid over an untrimmed
NURBS surface. `Mode=UV` divides normalized parameters; `Mode=Isocurve` divides
the U and V domain-start isocurves by arc length before forming the grid. Counts
of one use the corresponding domain start. `BasePoint=` is required, `Up=`
defaults to world Z, and `SurfaceName=` can be omitted when the target is the
last selected object. Surface normals determine orientation, while originals,
attributes, selection, and one copied group topology per cell are preserved.

`ArrayPolar` uses the same total-item and preservation rules around a top-view
center. Exactly 360 degrees omits a duplicate endpoint; other positive,
negative, and multi-turn sweeps include both endpoints. `Rotate=No` keeps object
orientation by orbiting the combined selection-bounds center, while `ZOffset`
adds a cumulative height per item.
