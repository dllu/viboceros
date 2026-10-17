# Helix and Spiral

Start `Helix` or `Spiral`, pick the axis start and end, then pick the radius
and starting direction. `Spiral` asks for a second radius. Snaps, typed world,
CPlane and relative points, coordinate filters and document length units use
the shared drafting pipeline.

Typing the first radius locks its size; a subsequent point sets the direction.
`Diameter` halves numeric and picked diameter values, and `Radius` switches back.
Enter at the first radius accepts the remembered size in the last hovered
direction. Enter at the second radius accepts its remembered value, initially
zero. Previews create no document objects; a completed curve is one undo step.
Escape, `Cancel`, or starting another command discards the pending construction.

`Turns` and `Pitch` accept either `Name=value` or a separate numeric prompt.
Turns accepts fractions; pitch accepts document units. Enter at a numeric
option retains its value. `Mode` toggles Turns/Pitch, or use `Mode=Turns` and
`Mode=Pitch`. `ReverseTwist` toggles direction; `ReverseTwist=Yes|No` assigns it.
The active mode controls what happens when the next axis length changes.
Settings are separate for Helix and Spiral, remembered after successful
interactive completion, and survive undo. Interactive commands start at ten
turns; full one-line scripts retain their existing three-turn default.

Choose `Vertical` before the first point to constrain the axis to the captured
CPlane normal. Pick its end, or enter a height and then pick above or below the
start to choose the side. Choose `Flat` before the first point of `Spiral` for
a planar spiral: pick its center and first radius, then the second radius.
Flat spirals have turns and twist, with no pitch. Their preview and committed
curve retain the captured plane when viewports change.

Choose `AroundCurve` before the first point, then explicitly select a whole
curve or a surface/Brep edge. Preselection is cleared for this getter. The
first radius pick establishes the starting direction in the rail's normal
plane; a numeric radius locks its size before that pick. `Helix` keeps that
radius, while `Spiral` asks for a second radius. Pitch uses the complete rail
length. `NumPointsPerTurn` accepts an integer of at least five, defaults to 12,
and supports both inline assignment and a separate numeric prompt.

A captured rail retains its original analytic representation. Geometry,
selectability or document tolerance changes invalidate it and return to rail
selection. Whole rails are selected during construction; edge picks leave
the parent unselected. Undo and Redo leave the source unselected and preserve
its geometry. Changing options or previewing a radius creates no objects.

Relative radius coordinates use the corresponding axis/rail endpoint, or the
Flat center, as their base. Relative vectors use the active CPlane axes. For
example, `r1,0,0` at the second radius of a vertical axis ending at `5,2,10`
means `6,2,10`. Flat spirals require different start and end radii.

Script forms include:

```text
Helix 0,0,0 0,0,10 2 Turns=3
Helix 0,0,0 0,0,10 2 Mode=Pitch Pitch=2.5 ReverseTwist=Yes
Spiral 0,0,0 0,0,6 1,0,0 4 Turns=2
Spiral Flat 1,2,3 2,2,3 4 Turns=2 Axis=0,1,0
Spiral AroundCurve 1,0,0 2 PathName=rail Turns=3 PointsPerTurn=12
Helix AroundCurve 1,0,0 Curve=OBJECT_ID Turns=2
Spiral AroundCurve 0,0,1 2 Edge=OBJECT_ID,0 Pitch=2
```

Replace `OBJECT_ID` with a document object ID. `PathName` must identify one
selectable curve; `Curve` and `Edge` are explicit selectors. Without a selector,
scripts use the last selected curve. AroundCurve scripts require a picked first
radius to seed direction. `PointsPerTurn` and `NumPointsPerTurn` are aliases.

The axial constructor reuses the linear tridiagonal cubic interpolator. Fresh
Rhino 8.32 captures replace the earlier 24/36-span assumption: below ten turns
it uses 120 spans per turn, then 60 below fifty, 36 below one thousand, and
24 at a thousand, with at least three spans. Its analytic endpoint tangents
and C2 interpolation are retained. Swept spirals use their explicit density,
equal-arc-length rail stations and shared adaptive frame transport. Their span count is the floor of density
times absolute turns, with a minimum of three. Closed rails produce periodic
cubics from the first N stations, omitting the terminal station, even for
fractional turns or unequal radii. A cyclic cubic solve keeps uniform periodic
interpolation linear in time and memory.

Axial input recipes verify complete control layouts, weights, knots, domains,
selection, lengths and actual undo/redo at `1e-6`. The sampling diagnostic corpus
also records the remaining endpoint-control differences for a radius of 100
and helices near a hundred and at a thousand turns; these exceed `1e-6` and
are not qualified at that threshold. Details and source hashes are in
[the validation record](../spiral-validation.json).

The additional rail and relative-coordinate corpus has 32 distinct successful
workflows and one native equal-radius Flat failure. Of the successful workflows,
31 agree in full geometry/history snapshots at `1e-6`. The closed spatial rail
has a maximum control distance near `1.97e-6`, bounded by `3e-6` as a diagnostic;
it is excluded from the `1e-6` qualifications. Native surface and box edge
captures include acknowledged physical clicks. Details are in the
[AroundCurve validation record](../spiral-around-validation.json).

Full mouse/snap parity and extreme parameter ranges remain unqualified.
Previewing a radius through Enter uses the local hovered direction; automated
native numeric/Enter captures without pointer receipts are not treated as
deterministic direction comparisons.

Reference: McNeel's public [Helix](https://docs.mcneel.com/rhino/8/help/en-us/commands/helix.htm)
and [Spiral](https://docs.mcneel.com/rhino/8/help/en-us/commands/spiral.htm) command documentation.
