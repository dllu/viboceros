# OffsetSrf

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/offsetsrf.htm)

Run `OffsetSrf`, select surfaces and press Enter. Preselected surfaces go directly
to the direction stage. Click an input or use `FlipAll` to reverse its direction.
Edit the distance, `Tolerance`, `Solid`, `BothSides`, or `Loose`, then press Enter to accept the cached
preview. `DeleteInput` is available with `Solid=Yes`. Esc cancels without changing
geometry or discarding Redo. The distance, tolerance and boolean choices survive cancellation;
direction flips apply only to the current invocation.

Type a signed distance, including units or arithmetic such as `1/8cm`.
`Distance` opens a positive-number or two-point prompt. Enter accepts a positive
default; a current zero distance uses default 1. Negative numbers stay at this
prompt, while zero exits without changing the value. Enter signed distances
directly at the direction stage.
`ThroughPoint` accepts the offset immediately after a point is supplied.
It computes the shared signed distance from the closest selected underlying
surface, including portions outside a face's trims. Other selected
objects use that distance and their own chosen directions.

`Tolerance` opens a nonnegative length prompt when `Loose=No`. Enter keeps the
current value; `Default` or zero follows document tolerance. One Esc cancels the
whole command. See [tolerance behavior and validation](../offset-srf-tolerance.md).

Complete command/API invocations also accept explicit inputs:

```text
OffsetSrf 2
OffsetSrf Distance=0.5cm Solid=Yes DeleteInput=Yes
OffsetSrf ThroughPoint=2,1,0.75 BothSides=Yes
OffsetSrf 0.5 Tolerance=0.0001
```

Copies use the current layer, remain unselected, and leave preselected inputs
selected. Selections made during the command clear on completion. `BothSides=Yes`
creates two separate open offsets. `Solid=Yes` builds a wall; combined with
`BothSides=Yes`, it spans both offsets. Nonzero solid `DeleteInput=Yes` replaces
each source's geometry while retaining its identity, attributes and groups.
`DeleteInput` has no effect on an open offset.

A zero open offset creates a copy. A zero one-sided wall returns coincident caps
and collapsed ruled sheets; deleting its input gives those pieces the source's
attributes and groups. A zero two-sided wall produces one B-rep containing two
oppositely oriented coincident faces. These finite rectangular cases match the
[native getter qualification](../offset-surface-getters.md).

The initial product preferences are distance 1, Sharp corners, and all booleans
No; tolerance follows the document. Factory Rhino defaults have not been established.
Round corners remain unsupported. Document changes invalidate a
preview; all geometry is prepared before one atomic history edit.
Returning to recent option combinations reuses validated preview geometry.

Both builds support planar sheets and exact canonical spheres, cylinders, cones
and ring tori, including supported single-face B-reps. Analytic offsets retain
their rational control nets and parameter domains; collapsing or inverting radii
are rejected. Planar rectangles make six-face walls, while other supported closed
profiles make exact ruled walls and caps. With `Loose=No`, portable builds reject other nonplanar
surfaces, polysurfaces and planar walls with multiple boundary loops.

`Loose=Yes` adds original-basis offsets of free-form single surfaces in both
builds, including supported trimmed faces, rational weights and periodic charts.
It interpolates normal offsets at Greville stations; the surface between those
stations approximates a true offset. See [Loose geometry and validation](../offset-srf-loose.md).
Loose is unavailable when an input has multiple faces or interior creases.

Linux `--features native-smlib` adds [usd-brep offsets](../smlib-offset-surfaces.md)
for general trimmed sheets, free-form surfaces and polysurfaces. Its broad corner,
collapse and self-intersection behavior remains separately qualified. The getter
captures establish finite workflows, not complete curved-surface parity.
Open single-face [trimmed offsets](../offset-srf-trims.md) preserve their UV loops
and holes, with certified spatial trim images on the fitted support surface.
