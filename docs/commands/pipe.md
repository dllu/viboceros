# Pipe

`Pipe [curve-id] start-radius [end-radius] [Stations=fraction:radius,...] [Cap=None|Flat|Round] [ShapeBlending=Local|Global] [FitRail=Yes|No] [Thick=Yes|No] [WallThickness=signed-distance]`
creates a circular-profile pipe around one selected curve. The first radius is
required and the second defaults to the first. The source curve remains in the
document. Enter `Pipe` without dimensions to pick a rail and radius point in a
viewport. `Pipe Cap=Round` also works in this picking flow. For example, after
selecting a line:

```text
Pipe 1.5 2.0 Cap=Flat ShapeBlending=Global
Pipe 1 WallThickness=0.25 Cap=Flat
Pipe 1 Cap=Round
Pipe 1 Stations=0.5:2 Cap=Flat
```

While picking, enter `Cap=`, `ShapeBlending=`, `FitRail=`, `Thick=`, or `WallThickness=` to
change those options. Cap and blending can also change after the first radius
of a thick pipe has been picked.

Straight constant-radius rails use exact cylinders. For two different radii,
`ShapeBlending=Local` uses a smooth cubic radius transition and
`ShapeBlending=Global` uses a cone. Global round caps use sphere segments
tangent to the cone; local round caps are hemispheres. A circular rail with one
radius creates a closed torus surface. Other
smooth open rails use one-rail Sweep1 with transported circular sections; flat
ends are capped as planar B-rep faces. `Cap=Round` joins spherical ends to
a single-wall pipe. On thick pipes, `Cap=Round` produces planar annular ends,
matching Rhino 8's Pipe command and `CreateThickPipe` result. `Cap=Flat` is the default.
The NURBS rail branch refits even for a constant radius: a cubic Bézier oracle
case differs by about 0.9% in volume when its original rail basis is retained,
while the refitted surface matches Rhino within `5e-6` model-volume units.
An independent two-span cubic NURBS case matches Rhino's flat Pipe volumes
within the same limit. Rhino's `FitRail=Yes|No` produced identical surfaces for
both tested NURBS rails; [Rhino's Pipe help](https://docs.mcneel.com/rhino/8/help/en-us/commands/pipe.htm)
describes that switch for line-and-arc polycurves.
On a tangent line-and-arc polycurve, `FitRail=No` (the default) joins separate
wall faces at the segment boundary; `FitRail=Yes` refits the full rail to one
wall face. The segment boundary radius uses the full rail's distance-weighted
smoothstep for both shape-blending modes; each segment then uses its chosen
Local or Global blend. Rhino comparison cases cover constant and tapered flat
pipes, a round cap, and a thick pipe, with matching face counts and volumes
within `5e-6` model-volume units.
For constant-radius open polylines and line-only polycurves, sharp corners use
exact mitered cylinder walls, including spatial chains with bends in different
planes. Two- and three-segment flat, round, and thick oracle cases match
Rhino's volumes within `5e-6` with both `FitRail=No` and `FitRail=Yes`.
`FitRail=No` planar and orthogonal three-segment cases also match face counts;
Rhino divides oblique spatial walls into more faces. With `FitRail=Yes`, Rhino
fits one wall face across the corners while this implementation keeps a face
per segment.
Closed constant-radius line loops with `FitRail=Yes` join the last miter back
to the first, including spatial loops whose two seam curves start at different
points. The fitted wall starts halfway along the first line and blends the
frame's accumulated rotation around the loop. Rectangle, triangle, and spatial
loop cases match Rhino's closed solid volumes within `5e-6` for both thin and
thick pipes. Ten points sampled from Rhino's spatial wall lie within `5e-4`
model units of the constructed wall. Both Rhino and this implementation use
one wall face around the full loop. Rhino returns two separate closed B-reps
for a thick closed pipe; this implementation combines the outer and inner
shells in one two-face B-rep.
End cap options have no effect on a closed rail.
For tapered sharp line rails with `FitRail=Yes`, the radius follows the full
rail distance: Local blending uses a cubic smoothstep profile, while Global
blending uses a linear profile. The mitered construction matches the tested
planar and spatial Rhino flat and thick volumes within `5e-6`; Local round
caps also meet that limit. Global round caps on horizontal tapered sharp rails
meet the same limit for the tested two-segment cases. Thick global pipes use
planar annular ends for `Cap=Round`, matching the tested spatial case. Thin
global round caps on nonhorizontal sharp rails remain unsupported: Rhino's
fitted caps change with rail orientation, and the tested spatial cases exceed
the volume tolerance.
The tested planar fitted taper agrees within `1e-4` in radius at six matched
axial positions. A global pointwise surface error bound remains open.
`WallThickness` adds a second wall and implies `Thick=Yes`; both options can be
entered explicitly. A positive thickness puts the second wall outside the first
radius; a negative thickness puts it inside. Flat caps join the walls with
annular faces. Straight constant-radius thick pipes use the exact tube B-rep;
curved thick pipes combine oppositely oriented swept walls. Circular thick pipes
use nested tori. In the viewport, `Pipe Thick=Yes` asks for two radius points.

`Stations=` supplies interior radius samples as normalized rail parameters in
strictly increasing order. For example, `Stations=0.25:1.5,0.75:2` sets radii
at one quarter and three quarters of the rail. Local blending fits a cubic
sweep through the stations; straight-rail cases with one or two interior
stations match Rhino's face counts and volumes within `5e-6`. The same wall
thickness applies at every station. Global blending with interior stations is
currently rejected; Rhino's fit changes intermediate cross sections.
On a tangent line-and-arc polycurve with `FitRail=No`, stations on a line
segment or at a segment boundary retain one wall face per original segment.
The radius at a segment boundary uses a smoothstep transition between the
nearest explicit stations. Stations inside a curved segment and all stations
with `FitRail=Yes` on a polycurve are currently rejected; the fitted Rhino
surface differs from the available sweep beyond the project volume tolerance.

This implements part of [Rhino's Pipe command](https://docs.mcneel.com/rhino/8/help/en-us/commands/pipe.htm).
Variable-radius sharp rails with `FitRail=No`, closed rails other than
constant-radius line loops and circles, and SubD output are still unsupported.
Closed sharp line loops with `FitRail=No` remain unsupported because Rhino's
tested rectangle case produces an open five-face result. A
circular rail requires a pipe radius smaller than the rail radius.
The Rhino 8 `CreatePipe` oracle likewise returned no geometry when an interior
radius station was supplied on a closed circular rail.
