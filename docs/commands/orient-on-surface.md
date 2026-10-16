# OrientOnSrf

`OrientOnSrf` (`OrientOnSurface`) places objects on a NURBS surface or an explicit
B-rep face. Start with selected sources, or select sources after starting the
command and press Enter. Pick the source base and reference points, then pick a
destination surface/face. Each placement asks for a target point, a positive
uniform scale, and a rotation angle in degrees. Enter accepts the current scale
or angle. A reference point can set the scale by distance from the target, and
a direction point can set the angle in its tangent plane.

`Copy=Yes` is the initial setting. Repeated copies always use the captured source
objects and share one Undo step. Enter at the target prompt or Escape finishes;
accepted copies remain. `Copy=No` replaces the sources and finishes after one
placement. Command-first source selection is released after successful placement.
The guide is never changed or selected by the placement. Rigid copies use the
shared transform group policy; a grouped single source gets an empty copied
group definition. Deformable copies preserve source group membership.

`Rigid=Yes` maps source and destination frames without bending the geometry.
`Rigid=No` uses the existing adaptive curve/surface/B-rep morph fitting and direct
mesh/point mapping. Both paths show temporary previews without editing the model.
The deformable preview shares immutable surface data and prepared display cages
between viewports. Wireframe, shaded, and ghosted views use the shared display
preview machinery.

The source Z direction comes from the construction plane of the view accepting
the source base. Set `OnSurface=Yes` before that point to select the guide first
and take source Z from its oriented surface normal. The reference defines source
X after projection into that source plane. `Flip=Yes` reverses the destination Y
and Z directions. A reversed B-rep face contributes its orientation to the flip.

`ConstrainNormal=Yes` is available with `Rigid=No`. It uses the construction-plane
normal of the viewport accepting the **target**, and freezes it while scale and
rotation are entered. This projects the anchor differential into that normal's
perpendicular plane and adjusts height by anchor obliquity. Tangent and nearly tangent projections
are rejected before model edits using the model angular tolerance and a
`1e-12` floor on the absolute normal dot product. Native command measurements also
show that flipped deformable placements keep unit source height even when their
in-plane scale differs from one. These command policies are separate from the
public generic Splop-style morph API.

Targets and OnSurface source bases use the surface-constrained point getter.
`IgnoreTrims=No` clamps picks to the selected face's trimmed region;
`IgnoreTrims=Yes` uses the underlying surface's natural domain. Changing, hiding,
locking, deleting the guide, or changing model tolerance invalidates its captured
reference and requests a new pick.

A complete scripted command uses model coordinates:

```text
OrientOnSrf 0,0,0 1,0,0 5,5,2 Surface=42 Face=0 Rigid=No Scale=1.4 Rotation=30 Copy=Yes
```

Use an actual object ID in place of `42`, or `SurfaceName=Panel` for a unique,
case-insensitive object name. The legacy last-selected guide convention is also
supported. `Sources=...` and `PickedSources=...` freeze sources for replay;
`SourceNormal=x,y,z` and `DestinationNormal=x,y,z` explicitly supply the two
normals for scripted use. CLI context supplies both defaults when omitted.

Validation records are in [orient-surface-validation.json](../orient-surface-validation.json).
The retained authored fixtures exercise planar/skew and rational cylindrical
surfaces, rigid and deformable placement, copy/in-place modes, source selection,
groups, repeated placements, source construction planes, OnSurface, face reversal,
flipped height, and constrained normals on planar and warped targets. Point-cloud
coordinates are compared to actual native command output at `1e-6`; these are
finite fixtures, not a proof for every surface or fitted object type. Existing
[curve](../curve-morphing.md), [surface](../surface-morphing.md), and
[B-rep](../brep-morphing.md) fitting qualifications still apply.

Whole-object sources are supported. Grip sources, nonuniform XYZ scaling,
Rhino's Scale/Rotation prompt-mode menus, and remembering non-copy settings
between separate invocations remain unimplemented. Trimmed guides and point
scale/rotation getters have local regression coverage; the retained native
fixtures do not qualify those interactions. No performance parity claim is made.

References: [Rhino 8 command help](https://docs.mcneel.com/rhino/8/help/en-us/commands/orientonsrf.htm),
[public SplopSpaceMorph API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.morphs.splopspacemorph/splopspacemorph).
Behavioral differences above come from owned native command experiments, without
inspection of proprietary implementation code.
