# Line Normal

Start `Line Normal`, or choose `Normal` at Line's first prompt. Select a curve,
surface or polysurface, then pick or type a start point. The getter projects
the start onto the reference and previews the constrained line before creating
one object on the current layer. `BothSides` mirrors the endpoint around the
projected start. Escape or Enter cancels an unfinished line.

For a curved curve, the endpoint lies along the principal normal. A signed
numeric length completes immediately: positive points opposite the curvature
vector. Surface normals follow face orientation, including reversed Brep faces.
For a straight curve or polyline segment, the endpoint can lie anywhere in the
plane perpendicular to its tangent. Here a positive number locks length and a
negative number sets a length increment; a following point chooses direction.

`IgnoreTrims=No` constrains the start to the trimmed face. `IgnoreTrims=Yes`
allows locations on its underlying surface, within that surface's domain.
With No, a polysurface start may move onto any of its faces; Yes retains the
picked face. The option starts at No for each new Line Normal command. Reference objects
remain unchanged; replacement, deletion, visibility, locking or tolerance
changes require another reference pick before completion.

Scripts can bind `Curve=id`, `Surface=id`, `Face=id,index` or `Edge=id,index`,
followed by a selection point, start point and endpoint or length. For example:

```text
Line Normal Curve=1 5,0,0 3,4,0 7,6,2
Line Normal Surface=2 5,5,0 4,3,2 5
```

Use current document object IDs. Interactive scripts and `ReadCommandFile` may
supply those tokens separately. Curve and surface mouse locations use the
original geometry; previews and final lines share the same construction model.
The source's identity, geometry storage, attributes and groups are preserved.
A completed line creates one Undo/Redo entry; cancellation retains pending Redo.

The [qualification record](../line-normal-validation.json) retains 53 native
captures: 46 complete workflows (39 constructions and seven cancellations), plus
seven input-driver diagnostics. Each Rust build checks 78 valid 3DM round trips
and compares geometry, domains, finite samples, selection and history at `1e-6`. Windowless egui events exercise source/base picking and
preview/commit agreement; lifecycle, units, calculator and command-file paths
have additional Rust checks. Run `python3 -m tools.rhino_oracle.check_line_normal`.

SubD references, native component-edge picking, arbitrary singular curves,
independent native PointAt samples, continuous distance and comparative speed
remain unqualified. [Perpendicular](line-perpendicular.md) supports sliding or fixed curve contacts. [Tangent](line-tangent.md) supports tangent contacts and common tangents.
The public [Rhino Line reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/line.htm)
describes the native command options.
