# Pipe

`Pipe [curve-id] start-radius [end-radius] [Cap=None|Flat] [ShapeBlending=Local|Global]`
creates a circular-profile pipe around one selected curve. The first radius is
required and the second defaults to the first. The source curve remains in the
document. Enter `Pipe` without dimensions to pick a rail and radius point in a
viewport. For example, after selecting a line:

```text
Pipe 1.5 2.0 Cap=Flat ShapeBlending=Global
```

Straight rails use exact cylinder or truncated-cone NURBS surfaces and capped
B-reps. A circular rail with one radius creates a closed torus surface. Other
smooth open rails use one-rail Sweep1 with transported circular sections; flat
ends are capped as planar B-rep faces. `Cap=Flat` is the default.

This implements part of [Rhino's Pipe command](https://docs.mcneel.com/rhino/8/help/en-us/commands/pipe.htm).
Rail corners, other closed rails, round caps, thick walls, extra radius stations,
and SubD output are still unsupported. A circular rail requires a pipe radius
smaller than the rail radius.
