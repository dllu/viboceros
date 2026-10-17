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

Existing script forms remain available:

```text
Helix 0,0,0 0,0,10 2 Turns=3
Helix 0,0,0 0,0,10 2 Mode=Pitch Pitch=2.5 ReverseTwist=Yes
Spiral 0,0,0 0,0,6 1,0,0 4 Turns=2
Spiral Flat 1,2,3 2,2,3 4 Turns=2 Axis=0,1,0
Spiral AroundCurve 1,0,0 2 PathName=rail Turns=3 PointsPerTurn=12
```

The axial constructor reuses the linear tridiagonal cubic interpolator. Fresh
Rhino 8.32 captures replace the earlier 24/36-span assumption: below ten turns
it uses 120 spans per turn, then 60 below fifty, 36 below one thousand, and
24 at a thousand, with at least three spans. Its analytic endpoint tangents
and C2 interpolation are retained. Swept spirals keep their separate explicit
sampling density and shared adaptive rail transport.

Native input recipes verify complete control layouts, weights, knots, domains,
selection, lengths and actual undo/redo at `1e-6`. The sampling diagnostic corpus
also records the remaining endpoint-control differences for a radius of 100
and helices near a hundred and at a thousand turns; these exceed `1e-6` and
are not qualified at that threshold. Details and source hashes are in [the validation record](../spiral-validation.json).

Interactive rail picking, Helix AroundCurve, full mouse/snap parity and extreme
parameter ranges remain unqualified. The existing scripted Spiral AroundCurve
workflow is unchanged. Previewing a radius through Enter uses the local hovered
direction; automated native numeric/Enter captures without pointer receipts are
not treated as deterministic direction comparisons.

Reference: McNeel's public [Helix](https://docs.mcneel.com/rhino/8/help/en-us/commands/helix.htm)
and [Spiral](https://docs.mcneel.com/rhino/8/help/en-us/commands/spiral.htm) command documentation.
