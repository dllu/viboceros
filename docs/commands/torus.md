# Torus

[Surfaces and solids](surfaces-index.md) · [Command input](../command-line.md)

Start the base circle with a center, or choose `Vertical`, `2Point`,
`3Point`, `Tangent`, `AroundCurve` or `FitPoints`. You can size a centered
base by radius, diameter, circumference or area. `Orientation` sets the
base plane. `ProjectOsnap` controls projection of snapped radius and
diameter picks. Select `Vertical` at either `2Point` prompt to constrain
the base plane.

The second circle sets the tube size. Its `Radius` and `Diameter` modes
have independent defaults from the base. At the tube prompt,
`FixInnerDimension` makes the base circle the inner rim. With that option,
a point pick sets the tube radius in radius mode or the full thickness
in diameter mode. Enter accepts the shown size. Torus remembers the base
size and tube settings as you answer the prompts.

The viewport previews the base circle, then the exact surface isocurves.
A successful Torus creates one closed rational NURBS surface in one Undo
step. The short form `Torus center major-radius tube-radius` uses two
explicit radii and accepts `Axis=x,y,z`.

```text
Torus 0,0,0 5 1.5
Torus 0,0,0 Diameter=10 FixInnerDimension=Yes Radius=1.5
Torus 3Point 5,0,0 0,5,0 -5,0,0 Radius=1.5
```
