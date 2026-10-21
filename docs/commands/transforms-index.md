# Transforms, placement and deformation

[All commands](README.md) · [Command input](../command-line.md)

- [Splop: sphere-sized surface wrapping and repeated copies](splop.md)
- [SetPt coordinate alignment for objects and grips](set-point.md)
- [World/CPlane bounding boxes](bounding-box.md)
- [Transforms and arrays](transforms.md)
- [Remap objects between construction planes](remap-cplane.md)
- [Interactive Orient and Orient3Pt point placement](orient.md)
- [OrientCrvToEdge](orient-curve-to-edge.md)
- [OrientOnCrv curve-guided placement and previews](orient-on-curve.md)
- [Independent reference scaling by plane](scale-by-plane.md)
- [Scale individual object positions](scale-positions.md)
- [Axis Twist deformation and rigid placement](twist.md)
- [Axis Stretch with numeric factors, lengths, targets and repeated copies](stretch.md)
- [Circular spine Bend deformation and rigid placement](bend.md)
- [Radial and flat Taper deformation](taper.md)
- [Cylindrical spiral Maelstrom deformation](maelstrom.md)
- [Move and repeated Copy placements](move-copy.md)
- [Selecting transform sources after starting a command](transform-sources.md)
- [Remember Copy options](remember-copy-options.md)
- [Object and group distribution](distribute.md)
- [Object and group alignment](align.md)
- [OrientOnSrf interactive surface placement](orient-on-surface.md)

- [Flow objects between curves](flow.md)

- [Flow objects between surfaces](flow-along-surface.md)

- [along curves](curve-array.md)

- [along curves on surfaces](array-curve-surface.md)

- [over surface grids](array-surface.md)

- [SoftEditSrf surface edits, periodic seams, constraints and previews](soft-edit-surface.md)

## Examples

These are independent snippets. Prepare the required sources and selection first;
replace object IDs, names and paths with values from your document. See each
command page for supported options and limitations.

```text
Move 0,0,0 5,0,0
Copy 5,0,0 5,5,0
Orient 0,0,0 1,0,0 5,5,0 5,8,0 Scale=1D Copy=Yes
Orient3Pt 0,0,0 1,0,0 0,1,0 5,5,0 5,8,0 4,5,1 Scale=No
OrientOnSrf 0,0,0 1,0,0 5,5,2 Rigid=No SurfaceName=Panel
Array 3 2 2 4 -3 5
Array 3 2 1 20 12 0 Mode=Fill
ArrayCrv 8 Orientation=Freeform PathName=Rail
ArrayCrvOnSrf 0,0,0 Curve=rail-id Surface=surface-id Divide=8
ArrayCrv Distance=2.5 Orientation=Roadlike BasePoint=0,0,0 PathName=Rail
ArraySrf 4 3 BasePoint=0,0,0 Up=0,0,1 Mode=Isocurve SurfaceName=Panel
ArrayLinear 4 0,0,0 2,1,0
Distribute XAxis Mode=Gap Spacing=Automatic
Align Concentric AlignTo=World Auto
ArrayPolar 6 0,0,0 360 Rotate=Yes ZOffset=0
Scale 0,0 2
Scale1D 0,0 2 1,0
Scale2D 0,0 2
ScaleNU 0,0,0 2 .5 1 Copy=Yes
ScalePositions 0,0,0 2 Mode=3D Copy=Yes
Rotate 0,0 45
Rotate3D 0,0,0 0,0,1 90 Copy=Yes
Mirror 0,-5 0,5
Shear 0,0,0 1,0,0 45 Copy=Yes
ProjectToCPlane DeleteInput=Yes
SetPt 0,0,5 XSet=No YSet=No ZSet=Yes Alignment=World
```
