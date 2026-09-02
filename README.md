# Viboceros

Viboceros is an early-stage, cross-platform CAD application written in Rust. It
is organized around independent geometry, document, drafting, command, and
file-format crates, with an egui interface rendered by wgpu.

The current foundation supports finite 3D points, vectors, line segments,
analytic circles, circular arcs, and ellipses, validated open and closed
polylines, planes, bounding boxes, rational NURBS curves with analytic first
derivatives and exact knot refinement, splitting, and interval trimming,
rational NURBS surfaces with analytic partial derivatives and exact tensor
splitting and rectangular domain trimming,
validated shared-topology B-reps with exact rational parameter-space trims,
validated mixed triangle/quad polygon meshes, layers, groups, and bounded
undo/redo.
Native point clouds preserve point order and duplicates, cache finite bounds,
and use a balanced XY spatial index for top-view snapping and picking.
The top viewport can pan and zoom in wireframe, shaded, or ghosted mode. Its
layer sidebar creates, renames, recolors, shows, locks, activates, and safely
deletes layers while reporting their object counts; combined edits remain one
undo step. The command line currently accepts:

```text
Point 1,2,0
Line 0,0,0 10,5,0
Circle 0,0,0 5
Arc 5,0,0 0,5,0 -5,0,0
Ellipse 0,0 6,0 0,3
Polyline 0,0 4,0 4,3 7,3
Rectangle 0,0 8,5
Polygon 6 0,0 5
Curve 0,0 2,3 5,3 8,0 Degree=3
ControlPointCurve 3 0,0 2,3 5,3 8,0
InterpCrv 0,0 1,2 4,-1 6,0 Knots=Chord Close=Open
SrfPt 0,0,0 8,0,0 8,5,2 0,5,2
PlanarSrf DeleteInput=No
Box 0,0,0 8,5,0 3
BoundingBox CoordinateSystem=World Cumulative=Yes Output=Solids
DupBorder OutputLayer=Current
DupEdge Edges=2,0 OutputLayer=Input
DupFaceBorder Faces=2,0 OutputLayer=Input
DupMeshHoleBoundary Boundaries=All
Sphere 0,0,0 5
Ellipsoid 0,0,0 5 3 2
Cylinder 0,0,0 5 10 Solid=Yes
Cone 0,0,0 5 10 Solid=Yes
Torus 0,0,0 5 1.5
Layer New Construction
Layer Hide Construction
Layer Show Construction
Layer Current Default
ChangeLayer Construction
CopyToLayer Default
SelAll
SelLast
SelPrev
SelLast DeselectOthersBeforeSelect=No
SelCrv
SelOpenCrv
SelClosedCrv
SelPlanarCrv
SelLine
SelPolyline
SelShortCrv 1.0
SelPt
SelPtCloud
SelSrf
SelPolysrf
SelOpenPolysrf
SelClosedPolysrf
SelMesh
SelOpenMesh
SelClosedMesh
SelColor 12,34,56
SelName "Fastener *"
SelLayer "Construction *"
SelGroup Assembly
SelDup
SelDupAll
Invert
Move 0,0,0 5,0,0
Copy 5,0,0 5,5,0
Orient 0,0,0 1,0,0 5,5,0 5,8,0 Scale=1D Copy=Yes
Orient3Pt 0,0,0 1,0,0 0,1,0 5,5,0 5,8,0 4,5,1 Scale=No
OrientOnSrf 0,0,0 1,0,0 5,5,2 Rigid=No SurfaceName=Panel
Array 3 2 2 4 -3 5
Array 3 2 1 20 12 0 Mode=Fill
ArrayCrv 8 Orientation=Freeform PathName=Rail
ArrayCrv Distance=2.5 Orientation=Roadlike BasePoint=0,0,0 PathName=Rail
ArraySrf 4 3 BasePoint=0,0,0 Up=0,0,1 Mode=Isocurve SurfaceName=Panel
ArrayLinear 4 0,0,0 2,1,0
ArrayPolar 6 0,0,0 360 Rotate=Yes ZOffset=0
Scale 0,0 2
Scale1D 0,0 2 1,0
Scale2D 0,0 2
ScaleNU 0,0,0 2 .5 1 Copy=Yes
Rotate 0,0 45
Rotate3D 0,0,0 0,0,1 90 Copy=Yes
Mirror 0,-5 0,5
Shear 0,0,0 1,0,0 45 Copy=Yes
ProjectToCPlane DeleteInput=Yes
ToNURBS DeleteInputObjects=Yes
ExtrudeCrv 5 BothSides=No DeleteInput=No
ExtrudeCrvToPoint 0,0,10 DeleteInput=No
ExtrudeCrvAlongCrv PathName=Rail DeleteInput=No
Revolve 0,0,0 0,0,1 270 StartAngle=0 DeleteInput=No
Group Assembly
Group All Everything
SetObjectName "Fastener Part" AppendCounter=Yes
SetObjectColor 12,34,56
SetObjectColor ByLayer
Ungroup
Ungroup Assembly
Join
Explode
Length
Area
Volume
Divide 8
Divide Length 2.5 MarkEnds
CrvStart
CrvEnd
ExtractPt OutputLayer=Input Output=Points
ExtractPt OutputLayer=Input Output=PointCloud
ExtractControlPolygon OutputLayer=Current
ExtractSrf 2,1,0 Copy=No OutputLayer=Input
ExtractSrf Faces=0,2 Copy=Yes OutputLayer=Current
ExtractIsocurve 2,1,0 Direction=Both
ExtractIsocurve ExtractAll Direction=Both IgnoreTrims=No
ExtractWireframe OutputLayer=Current GroupOutput=No
ConvertToSingleSpans Direction=Both DeleteInput=No
ConvertToBeziers DeleteInput=No
CloseCrv
CloseCrv CloseWideGapsWithLine=No Tolerance=0.01
Flip
UnifyMeshNormals
CombineIdenticalMeshVertices
CullUnusedMeshVertices
SplitDisjointMesh
ExtractDuplicateMeshFaces
ExtractNonManifoldMeshEdges
ExtractNonManifoldMeshEdges ExtractHangingFacesOnly=Yes MinimumFaceCount=3
Hide
Show
Lock
Unlock
HideSwap
LockSwap
Isolate
Unisolate
IsolateLock
UnisolateLock
Delete
Clear
Undo
Redo
ImportStl path/to/model.stl
ExportStl Binary path/to/model.stl
ExportStl Ascii path/to/model.stl
ImportStep path/to/model.step
ExportStep path/to/model.step
Import3dm path/to/model.3dm
Export3dm path/to/model.3dm
Help
```

Enter `Point`, `Line`, `Circle`, `Arc`, `Ellipse`, `Polyline`, `Curve`,
`InterpCrv`, `Rectangle`, `Polygon`, `SrfPt`, `Sphere`, or `Ellipsoid` without
coordinates to pick points in the viewport; press Enter to finish a polyline or curve. `Curve`
creates an open control-point curve, defaults to degree three, accepts degrees
through 11, and lowers the degree when too few controls are supplied.
`Close=Smooth` creates a periodic seam (degree one remains linear and closed);
`Close=Sharp` repeats the first control to create a kinked, non-periodic seam.
`InterpCrv` defaults to an open, degree-three, chord-knot curve through the
input points.
It also supports `Degree=1`, `Knots=Uniform|SqrtChrd`, smooth periodic or sharp
`Close=` modes, and start/end tangent directions on open cubics. `Polygon`
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
`ToNURBS` exactly converts selected lines, circles, arcs, ellipses, and
polylines. It retains inputs and creates unselected copies in the same groups
by default; use `DeleteInputObjects=Yes` to preserve object identities and
replace the inputs in place. Line, polyline, circle, arc, and ellipse parameter
domains follow Rhino's chord-length, arc-length, and angular conventions.
`PlanarSrf` creates exact single-face B-reps from selected closed planar curves.
Wholly contained curves become inner trim loops, alternating nested regions
become independent islands, and partially overlapping curves remain separate
surfaces. Every original rational curve remains an exact boundary edge and
parameter-space trim, so curved, concave, and holed regions shade and export
without filling excluded areas. Inputs are retained by default;
`DeleteInput=Yes` removes each successfully used boundary.
`Sphere` creates Rhino/OpenNURBS' exact 9-by-5 rational quadratic surface from
a center and numeric radius or point on the sphere. Its longitude domain is
`[0, 2π]`, its latitude domain is `[-π/2, π/2]`, and entering it without
arguments starts the two-pick viewport workflow.
`Ellipsoid` creates the exact closed 9-by-5 rational surface obtained by
scaling that sphere along three orthogonal semi-axes. Supply three numeric
radii in World XY, or center and three axis-radius points for an oriented
ellipsoid; entering it without arguments starts the four-pick viewport workflow.
`Box` follows Rhino's default two-opposite-base-corners and height workflow in
World XY. The height may be a signed number or a point, and the result is one
closed B-rep with eight shared vertices, twelve shared edges, six outward
bilinear faces, and exact rational parameter-space trims.
`BoundingBox` creates one cumulative World-coordinate enclosure of the selected
objects by default; `Cumulative=No` creates one per object. `Output=` supports
exact B-rep solids, shared-vertex triangle meshes, six grouped rectangle curves,
or report-only `None`. World-plane selections produce a rectangle or mesh plane.
`CoordinateSystem=CPlane` is accepted and currently shares the World XY basis;
analytic bounds are tight, while NURBS bounds conservatively enclose their
positive-weight control geometry.
`DupBorder` duplicates the open boundaries of selected NURBS surfaces, B-reps,
and triangle meshes. Surface borders are exact rational isocurves, including
at non-clamped domain ends; closed seams and collapsed singular sides are
omitted. B-reps preserve each exact naked edge, while mesh edges are welded by
exact location into boundary polylines. Multi-edge borders are grouped until
the document gains an exact polycurve primitive. Results use the current layer
and become selected by default; `OutputLayer=Input` uses each source layer.
`DupEdge` duplicates the exact edge nearest a model-space point, or accepts an
ordered zero-based `Edges=0,2`/`Edges=All` selector for every selected NURBS
surface, B-rep, or mesh. Standalone closed-surface seams remain selectable,
collapsed singular sides are omitted, B-rep rational edge curves stay exact,
and mesh indices follow exact-location-welded topology order. Omit the selector
for a one-pick viewport workflow. Fresh selected results default to the current
layer; `OutputLayer=Input` uses each source layer.
`DupMeshHoleBoundary` duplicates the closed naked loop nearest a model-space
point on selected polygon meshes, or accepts ordered zero-based
`Boundaries=0,2`/`Boundaries=All` selectors for every selected mesh. Boundaries
use exact-location-welded topology, remain closed polylines in deterministic
topology order, and fresh selected results use the current layer. Omit the
selector for a one-pick viewport workflow.
`DupFaceBorder` duplicates the exact non-seam border of the nearest selected
surface or B-rep face, or accepts ordered zero-based `Faces=0,2`/`Faces=All`
selectors. Omit the selector for a one-pick viewport workflow. Linear edge
chains become one closed polyline; curved multi-edge chains retain exact NURBS
segments in a group until polycurves are available. Holes and disconnected
borders remain separate, singular and seam trims are omitted, and fresh
selected results default to the current layer (`OutputLayer=Input` is also
supported).
`ExtractControlPolygon` fits degree-one polylines through the Euclidean controls
of selected curves and creates mixed triangle/quad meshes through selected
untrimmed NURBS surface control nets. Periodic control windows align to the
active domain, closed seams remain explicit, and singular surface sides become
triangles without artificial quad diagonals. Results default to the current
layer; `OutputLayer=Input` and `TargetObject` use each source object's layer.
`ExtractSrf` separates the nearest exact face at a model-space point, or accepts
an ordered zero-based list such as `Faces=0,2` (`Faces=All` is also supported).
The face list applies to every selected NURBS surface or B-rep; entering the
command without a selector starts a one-pick viewport workflow. `Copy=No` is
the default: an unextracted B-rep remainder keeps the source identity,
attributes, and groups, while a fully extracted source is deleted. Extracted
faces preserve attributes, become selected independent objects, and never
inherit source groups. `Copy=Yes` retains the source. Output defaults to the
input layer; `OutputLayer=Current` changes only the result layer.
`ExtractIsocurve` creates the exact U, V, or both rational isocurves nearest a
model-space point on every selected NURBS surface or B-rep. B-rep results come
from the nearest trimmed face and are split exactly around outer boundaries and
holes; rational p-curve intersections determine the retained parameter
intervals without faceting the output. Extracted curves preserve the varying
direction's degree and parameter values even at non-clamped spans. Omit the
point to pick a surface location in the viewport. `IgnoreTrims=Yes` uses the
full underlying B-rep face instead. `ExtractAll` emits the natural boundaries,
knot wires, and density-dependent wires inside each knot span from all selected
surfaces or B-rep faces; the per-object Rhino wire density survives 3DM I/O.
The same wire-density rules drive viewport display and `ExtractWireframe`.
That command emits each B-rep or exact-location-welded mesh topology edge once,
adds exact trim-clipped interior surface isocurves, selects the results, and can
place them on the current or input layer. `GroupOutput=Yes` forms one output
group; `OutputLayer=TargetObject` currently uses the selected target's layer.
`ConvertToSingleSpans` decomposes selected untrimmed NURBS surfaces at their
exact knot spans in `Direction=U`, `V`, or `Both`. Rational weights, source
parameter values, attributes, and group membership are preserved. Inputs are
retained by default; `DeleteInput=Yes` replaces them in one undoable edit, and
surfaces already single-span in the requested direction are left untouched.
`ConvertToBeziers` decomposes selected NURBS curves and untrimmed NURBS surfaces
exactly at every nonempty knot span. It preserves rational weights, source
parameters, attributes, and group membership. Inputs are retained by default;
`DeleteInput=Yes` replaces them with fresh pieces in one undoable edit. A
single-span input still produces a fresh Bezier object.
`Cylinder` creates an exact 9-by-2 rational NURBS wall from a center, radius
(or base-circle point), and signed height. `Axis=` accepts an arbitrary axis,
while `BothSides=Yes` makes the height symmetric about the base center.
`Solid=Yes` adds exact rational polar-disk caps and shared rim/seam topology as
a closed B-rep; `Solid=No` retains the single open wall surface.
`Cone` uses the same center, numeric radius or base-point radius, signed height,
and `Axis=` conventions. It produces Rhino/OpenNURBS' exact 9-by-2 rational
wall with a weighted collapsed apex. `Solid=Yes` adds an exact polar-disk base
and shared rim/seam topology as a closed B-rep; `Solid=No` leaves the wall open.
`Torus` creates an exact closed 9-by-9 rational quadratic surface from a center,
major radius (or point on the major circle), and minor radius. `Axis=` orients
the torus; the major radius must exceed the positive minor radius. Its U and V
domains are the major- and minor-circle circumferences, matching OpenNURBS.
`ExtrudeCrv` creates exact rational NURBS surfaces from selected analytic,
polyline, and NURBS curves. A numeric distance uses World Z; two points define
an arbitrary direction; entering it without either starts the two-pick viewport
workflow. `BothSides=Yes` sweeps equally in both directions and
`DeleteInput=Yes` removes the profiles. Matching Rhino, outputs are unselected,
ungrouped objects with fresh attributes on the current layer. `Solid=Yes`
turns each closed planar profile into an exact closed B-rep with a ruled wall,
two planar trimmed caps, shared rational rim curves, and one shared wall seam;
open and nonplanar profiles remain exact NURBS surfaces.
`ExtrudeCrvToPoint` creates exact rational NURBS surfaces that taper selected
curves to one apex. Its U direction runs from the profile to the apex while V
preserves the source curve's degree, knots, and weights, matching Rhino's NURBS
form. Enter it without an apex to pick one in the viewport. Inputs are retained
by default; `DeleteInput=Yes` removes them. `Solid=Yes` turns each closed planar
profile into an exact closed B-rep with a singular-apex ruled wall, planar
trimmed cap, shared rational rim, and one twice-used wall seam; open and
nonplanar profiles remain surfaces. `Output=Surface` and `Solid=No` are accepted
explicitly; SubD output is not yet represented.
`ExtrudeCrvAlongCrv` creates an exact fixed-orientation sum surface from each
selected profile and a curve path. Name the path with `PathName=`, or select it
last; the toolbar uses the last-selected convention. The path is retained and
deselected. `DeleteInput=Yes` removes only successfully extruded profiles.
Profile degree, knots, and weights become U; path data becomes V; tensor weights
are multiplied exactly. With an open path whose endpoints leave the profile
plane, `Solid=Yes` turns each closed planar profile into an exact capped B-rep
with translated planar trims, shared rational rims, and an exact copy of the
path as the twice-used wall seam. Open and nonplanar profiles, or profiles swept
along a closed path, remain surfaces. `Output=Surface` and `Solid=No` are
accepted explicitly.
`Revolve` creates exact rational NURBS surfaces around an arbitrary two-point
axis. Supply a signed sweep from -360 through 360 degrees, or use
`FullCircle=Yes`; `StartAngle=` rotates the beginning of a partial sweep.
Entering `Revolve` without an axis starts the two-pick viewport workflow, which
defaults to a full turn and also accepts `Angle=`, `StartAngle=`, and
`DeleteInput=`. The exact surface keeps the profile in V and uses fully
multiple quadratic quadrant knots in U. `Output=Surface` and `Deformable=No`
are supported; deformable and SubD revolves are not yet represented.
`ArrayPolar 6` picks a
center for a 360-degree top-view array; an optional angle and `Rotate`/`ZOffset`
settings can follow the count. `Array 3 2` picks two top-view corners for a
rectangular array. A third count adds Z levels and requires `ZDistance=...`;
`Mode=Fill` treats the picked rectangle as the outside array span. `Join`
connects unambiguous line/polyline endpoint chains within the document
tolerance. `Explode` turns polylines into line segments, frees point-cloud
members as points, duplicates polysurface faces as exact trimmed B-reps, and
splits meshes at disconnected or unwelded edges. Parts are emitted in Rhino's
reverse component order, inherit attributes, and replace their source in every
existing group. `Length` measures
analytic, polyline, and NURBS curves with controlled accuracy; `Area` measures
circles, ellipses, closed planar polylines, exact NURBS surfaces, B-reps, and
meshes. Full-domain NURBS faces are integrated per knot-span rectangle, while
planar trimmed B-rep faces use their exact boundary integrals, including inner
holes; general nonplanar trims remain explicit errors.
`Volume` reports the accumulated signed volume of selected closed triangle
meshes and exact B-rep solids; outward orientation is positive and reversed
orientation is negative. Meshes use translation-stable tetrahedral
accumulation. Full-domain NURBS B-rep faces are integrated directly over each
knot-span rectangle with adaptive quadrature, without measuring a display
tessellation. Planar trimmed caps use an exact-edge boundary-area integral;
general nonplanar trims are rejected until constrained trimmed-domain mass
properties are implemented. Measurement does not alter history.
`Divide` creates equal arc-length points on selected curves by segment count or
requested segment length; add `MarkEnds` to include open-curve endpoints.
`CrvStart` and `CrvEnd` place attribute-preserving point objects at the natural
ends of every selected curve. `CloseCrv` currently closes selected polylines:
nearby ends are made identical, while wider gaps gain a straight segment unless
`CloseWideGapsWithLine=No` is set. Lines that cannot form a valid three-segment
loop remain unchanged. Open arcs and NURBS curves will be enabled once the
document has an exact polycurve representation.
`ExtractPt` duplicates curve controls, surface control nets, and every raw mesh
vertex (including unused and coincident vertices). Closed seams and periodic
NURBS control rings follow Rhino ordering. Point results default to each input
layer; `OutputLayer=Current` puts them on the active layer with fresh
attributes. `Output=PointCloud` merges the locations into one cloud in selection
order, and selects the result while deselecting the sources. Input-layer output
copies the first selected source's attributes when that source contributes
locations; otherwise it uses fresh current-layer attributes.
`Hide` and `Lock` change selected objects; `Show` and `Unlock` restore every
object with the corresponding object-level state. Hidden objects neither render
nor snap. Locked objects render in gray and remain available to osnap, but
cannot be selected or edited. Layer visibility and locking remain independent.
`HideSwap` exchanges normal and hidden object modes, while `LockSwap` exchanges
normal and locked modes. Like Rhino, both swaps affect only objects on visible,
unlocked layers and leave the third object mode unchanged.
`Isolate` hides ordinary objects outside the selection and `IsolateLock` locks
them; objects already hidden or locked and objects on hidden or locked layers
are unchanged. Their `Unisolate` counterparts restore only modes introduced by
the matching isolate command, with provenance preserved through undo and redo.
Rhino-compatible curve, line, polyline, point, point-cloud, surface, and
polysurface, and open/closed mesh filters add visible, unlocked objects of the
requested type to the current selection. `SelPtCloud` is separate from `SelPt`,
matching Rhino. `SelSrf` includes both untrimmed NURBS surfaces and single-face
trimmed B-reps while excluding multi-face B-reps. `SelPolysrf` (alias
`SelPolysurface`) and its open/closed variants classify only multi-face B-reps
by shared-edge topology.
`SelPlanarCrv` uses document tolerance. `SelLine` also recognizes
exactly straight, single-span higher-degree NURBS curves, while excluding
multi-span curves and polylines as Rhino does. `SelPolyline` includes native
polylines and multi-segment degree-one NURBS curves, but excludes line objects
and two-control-point degree-one NURBS curves. `SelShortCrv` takes an explicit
positive maximum length and includes curves exactly on that boundary; it uses
the same controlled length calculation as `Length`. Mesh closure uses exact
location-welded polygon-edge topology, so quad meshes, indexed triangle meshes,
and STL-style triangle soup classify consistently; quad diagonals are used only
when an operation explicitly needs triangles.
`SelLast` selects every object changed by the latest object-editing transaction,
including multi-object imports and command outputs. `SelPrev` swaps the current
and previous selection sets. Both replace by default, matching Rhino; set
`DeselectOthersBeforeSelect=No` to add instead.
`SelName` and `SelLayer` add case-insensitive `*`/`?` wildcard matches without
expanding overlapping groups; `SelName ""` selects unnamed objects. `SelGroup`
uses Rhino's exact, case-sensitive group names. Matching hidden or locked layers
with `SelLayer` makes those layers visible and unlocked outside undo history,
while object-level hidden and locked states remain untouched.
`SelColor r,g,b` adds visible, unlocked, ungrouped objects with that resolved
display color; as in Rhino, objects contained in groups are skipped. ByLayer
objects use their layer color; material- and parent-sourced objects currently
use the same documented fallback as the viewport.
`SelDup` adds every visible, unlocked geometric copy except one deterministic
document-order original per class; `SelDupAll` includes those originals.
Equality is independent of object properties, groups, and document tolerance,
and direction-independent for lines, open polylines, analytic circles and
arcs, and compatible NURBS curves. Points and indexed meshes compare exact
stored values; curves use Rhino/OpenNURBS' scale-aware fixed zero policy.
Closed piecewise-linear and NURBS seams remain significant, while mesh vertex
indexing, face order, and winding must match.
`SetObjectName` assigns one shared name to the selection. Add
`AppendCounter=Yes` for Rhino's zero-based suffixes in document order, or use
`SetObjectName ""` to clear names. Unnamed `Group` calls receive Rhino-style
`Group01`, `Group02`, ... names; explicit group names are case-sensitive.
`SetObjectColor r,g,b` assigns Rhino-style per-object display color to the
selection; `SetObjectColor ByLayer` restores layer-driven display while
retaining the stored object color. Selection and locked-object colors still
take visual precedence. Material- and parent-sourced colors are preserved in
3DM files and currently fall back to layer color until materials and instance
definitions are implemented.
`ChangeLayer` moves selected objects without changing their identities, groups,
or the current layer. `CopyToLayer` skips selected objects already on the target,
copies each remaining group subset into a fresh automatic group, and leaves the
original selection unchanged. Hidden and locked target layers are supported.
`Orient` maps two reference points to two target points with Rhino's shortest
3D rotation. `Scale=No` preserves size, `Scale=1D` changes only the reference
axis, and `Scale=3D` scales uniformly. `Orient3Pt` maps right-handed frames
defined by three reference and target points; its `Scale=Yes` factor comes from
the first point pair. Both commands default to in-place, unscaled transforms;
`Copy=Yes` preserves the original selection, attributes, and group topology.
`OrientOnSrf` uses a source base/reference pair and a point nearest the target
NURBS surface. Name that surface with `SurfaceName=`, or make it the last
selected object. `Rigid=Yes` (the default) maps a frame without deformation;
`Rigid=No` applies Rhino's plane-to-surface morph; lines and polylines become
cubic deformable curves, analytic and NURBS curves receive tolerance-driven
adaptive cubic fits, and meshes map per vertex. `Scale=` must be positive,
`Rotation=` is in degrees, `Flip=Yes` reverses surface Y and Z, and
`SourceNormal=` defaults to world Z. In deformable mode, `ConstrainNormal=Yes`
keeps normal offsets parallel to `SourceNormal`, the command-line stand-in for
Rhino's placement-viewport construction-plane normal.
This command defaults to `Copy=Yes`; originals remain selected and copied group
topology is preserved. Use `Copy=No` for an identity-preserving in-place morph.
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
`Flip` on the rail to reverse its travel direction.
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
`UnifyMeshNormals` repairs inconsistent face winding across exact
location-welded manifold edges, including STL-style triangle soup, and rejects
non-orientable constraints atomically. `Flip` (aliases `Reverse` and `Rev`)
reverses selected curve directions or every face in selected meshes without
changing object identities, attributes, groups, or closed-curve seams.
`CombineIdenticalMeshVertices` turns exactly coincident raw vertices into shared
indices without culling unused vertices, preserving object identity, metadata,
face order, and winding.
`CullUnusedMeshVertices` removes raw vertices not referenced by any face while
preserving the source order and identity of every referenced vertex, including
coincident vertices, as well as object metadata, face order, and winding.
`SplitDisjointMesh` separates exact-location edge-connected components (a lone
shared vertex does not connect them), retaining the first object identity and
copying attributes and group membership to additional pieces.
`ExtractDuplicateMeshFaces` separates all but one face from each duplicate
class. Face equality uses exact vertex locations and ignores raw indices,
cyclic ordering, and winding; attributes and group membership are preserved.
`ExtractNonManifoldMeshEdges` removes faces around edges shared by three or
more faces into attribute- and group-preserving mesh objects. Options can limit
the operation to hanging faces or raise the minimum incident-face count.
Osnap captures visible Point, End, Mid, Center, and Quad features, including
indexed members of point clouds and features on locked objects and layers;
SmartTrack captures horizontal and vertical alignment from the first picked
point. Drag with the middle mouse button to pan while a drafting command is
active. Outside a drafting command, click geometry to select its connected
group, Shift-click to add, and Ctrl-click or Command-click to toggle. Click
empty space or press Esc to clear the selection; press Delete to remove
selected objects.

## Build and run

Install a current stable Rust toolchain (Rust 1.95 or newer), CMake, and a C++17
compiler. Initialize the pinned OpenNURBS submodule after cloning, then run:

```sh
git submodule update --init --recursive
cargo run
```

On Linux, wgpu uses Vulkan when available and the window supports both Wayland
and X11. Run all workspace tests and checks with:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Rhino compatibility oracle

The versioned Python oracle API runs identical JSON geometry and document-state
batches in a native release build of Viboceros and Rhino 8, recursively checks
results, and reports per-operation timings. With Rhino installed through the
configured Wine/FEX launcher, run the core fixture with:

```sh
python3 -m tools.rhino_oracle compare \
  tools/rhino_oracle/fixtures/core.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12
```

The same workflow is importable for instrumentation and tests:

```python
from tools.rhino_oracle import OracleClient, load_request

report = OracleClient().compare(load_request("tools/rhino_oracle/fixtures/core.json"))
assert report.passed
```

Set `VIBOCEROS_RHINO_LAUNCHER` to use another launcher. McNeel's documented
`/runscript` startup interface is tried first; this project's Wine path also
has an owned-window fallback that requires `wmctrl` and `xdotool` and never
targets a pre-existing Rhino process. Set `VIBOCEROS_RHINO_UI_FALLBACK=0` to
disable it. The `viboceros` and `rhino` modes run either side independently.
Timings cover repeated API calls after one warm-up and exclude process startup,
fixture construction, and JSON I/O; Rhino timings include its public
Python/RhinoCommon bridge.

Both ASCII and binary STL are supported. 3DM import/export uses McNeel's
OpenNURBS toolkit and preserves points, point-cloud locations, lines, NURBS
curves, untrimmed NURBS surfaces, mixed triangle/quad meshes, and editable
rational NURBS B-reps. Mesh faces retain their arity in 3DM round trips. B-rep
interchange retains shared vertices and edges, exact edge and
parameter-space trim curves, face surfaces and orientation, outer and inner
loops, boundary/mated/seam/singular trims, and modelling tolerances. Layer and
object state are also preserved, including the raw RGB display color, its
layer/object/material/parent source, and surface wire density. Named group
definitions and ordered membership survive round trips, including overlapping
and empty groups.
Circles, arcs, ellipses, and polylines are exported without approximation as
rational NURBS curves; canonical degree-one curves return as editable
polylines. Unsupported object types and specialized B-rep trim forms are
counted and reported during import.

Initial STEP interchange uses the Apache-2.0 Monstertruck kernel to read
solid/shell B-reps and assemblies, apply instance transforms, and robustly
tessellate exact trimmed surfaces into validated display meshes. Parser,
topology, and unsupported-representation losses are reported instead of being
silent. STL and STEP export tessellate visible NURBS surfaces plus full-domain
and trimmed planar B-rep faces; exact outer and inner p-curves are sampled into
a constrained triangulation so holes remain open. STEP writes the results as
faceted shells with shared topology and planar faces. Nonplanar general trims
remain explicit errors rather than being silently filled. Editable STEP B-rep
interchange and production surface and solid modelling are not implemented yet.
