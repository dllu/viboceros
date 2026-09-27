# Pipe

`Pipe [curve-id] start-radius [end-radius] [Stations=fraction:radius,...] [Cap=None|Flat|Round] [ShapeBlending=Local|Global] [Thick=Yes|No] [WallThickness=signed-distance]`
creates a circular-profile pipe around one selected curve. The first radius is
required and the second defaults to the first. The source curve remains in the
document. Enter `Pipe` without dimensions to pick a rail and radius point in a
viewport. For example, after selecting a line:

```text
Pipe 1.5 2.0 Cap=Flat ShapeBlending=Global
Pipe 1 WallThickness=0.25 Cap=Flat
Pipe 1 Cap=Round
Pipe 1 Stations=0.5:2 Cap=Flat
```

Straight constant-radius rails use exact cylinders. For two different radii,
`ShapeBlending=Local` uses a smooth cubic radius transition and
`ShapeBlending=Global` uses a cone. Global round caps use sphere segments
tangent to the cone; local round caps are hemispheres. A circular rail with one
radius creates a closed torus surface. Other
smooth open rails use one-rail Sweep1 with transported circular sections; flat
ends are capped as planar B-rep faces. `Cap=Round` joins spherical ends to
a single-wall pipe. On thick pipes, `Cap=Round` produces planar annular ends,
matching Rhino 8's Pipe command and `CreateThickPipe` result. `Cap=Flat` is the default.
`WallThickness` adds a second wall and implies `Thick=Yes`; both options can be
entered explicitly. A positive thickness puts the second wall outside the first
radius; a negative thickness puts it inside. Flat caps join the walls with
annular faces. Straight constant-radius thick pipes use the exact tube B-rep;
curved thick pipes combine oppositely oriented swept walls. Circular thick pipes
use nested tori. In the viewport, `Pipe Thick=Yes` asks for two radius points.

`Stations=` supplies interior radius samples as normalized rail parameters in
strictly increasing order. For example, `Stations=0.25:1.5,0.75:2` sets radii
at one quarter and three quarters of the rail. Local blending transitions
smoothly between each pair of stations. The same wall thickness applies at
every station. Global blending with interior stations is currently rejected;
Rhino uses a different global fit for three or more radius samples.

This implements part of [Rhino's Pipe command](https://docs.mcneel.com/rhino/8/help/en-us/commands/pipe.htm).
Rail corners, other closed rails, and SubD output are still unsupported. A
circular rail requires a pipe radius smaller than the rail radius.
