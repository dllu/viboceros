# OrientOnCrv

Place selected objects along a curve with a rigid transform. `OrientOnCurve` is
an alias. Start the command, select the source objects if necessary, pick a base
point, select an orientation curve, and pick the new base point on that curve.
`OnCurve` at the first prompt selects the guide before picking the source base.

At the destination prompt, `Copy=Yes` permits repeated placements. Enter or
Escape keeps accepted copies and ends the command; a single Undo removes the
batch. Copy follows `RememberCopyOptions`, with an initial default of No.
`Flip` reverses the guide used to calculate the destination orientation.
`Perpendicular` makes the object's local Z axis follow the guide tangent and
enables `XFlip`, `YFlip`, and `Rotate=Yes`. Rotation then asks for an angle in
degrees or two reference points after the destination has been picked. Enter
accepts the previous successful angle.

Complete commands use an explicit guide ID and optional numeric rotation:

```text
OrientOnCrv 0,0,0 0,5,0 Curve=<object-id> Copy=Yes
OrientOnCrv 0,0,0 0,5,0 Curve=<object-id> Perpendicular=Yes XFlip=Yes Rotation=30
```

Options accept Yes/No values. The base construction plane is frozen when the
base is accepted. OnCurve uses the original guide frame at the base. Planar
curves use their oriented supporting plane, queried through public OpenNURBS;
spatial curves use the existing Rust rotation-minimizing frame transport.
Flip recalculates frames from the reversed guide, including closed winding.
Mouse placement uses the existing screen-space curve picker in all four views.
The cursor preview shares the command solver and does not edit document objects.

At the guide prompt, `SubCrv` selects a temporary portion: pick the curve, then
its start and directed end. A decreasing open interval reverses the guide; a
decreasing closed interval crosses its seam. Instead of an end point, enter a
length and confirm its direction on the curve. `D` or `Direction=Locked` uses
the current cursor direction; `Direction=Free` unlocks it. Numeric shortening
retains the source direction on an open curve, including backward shortening.
Zero length or Enter returns to guide selection; Escape cancels the parent.
The original curve and its groups stay unchanged, and no temporary objects or
history entries are added to the document. Inline SubCrv clears source selection,
while the command keeps its captured source IDs for repeated copies.

Scripts use original-curve parameters, for example:

```text
OrientOnCrv 0,0,0 0,5,0 Curve=<object-id> SubCrv=2,8 Copy=Yes
```

A shortened spatial guide starts its frame transport at its new start. The
private guide also constrains cursor placement and clamps typed destinations.
Relative input such as `rw0,-1,0` starts from that constrained location.
See [SubCrv validation and current limits](../orient-on-curve-subcurve.md).

The 35 primary and 14 spatial captures from Rhino 8.32 match all four witness
points within `1e-6`, with matching source/copy selection and unchanged guides.
They cover construction planes, lines, circles, a polyline corner, spatial
cubics, OnCurve, flips, numeric rotation, and an off-curve typed target. These
fixtures do not establish full Rhino parity or general performance. Using the
guide itself as a source and selecting surface edges as guides remain unsupported. Native
reference-point rotation and repeated-copy undo have not been captured; those
paths currently have local tests. A further 32 captures cover temporary portions,
clamping, and relative copy batches; all 81 command-backed comparisons agree
within `1e-6`.

The initial discovery capture used an extra Enter and canceled all three cases;
the placement capture also contains one canceled rotation with incorrect macro
ordering. These are retained as rejected setup diagnostics and excluded from
the 49-case comparison. SDK frames in the successful captures are diagnostics;
the local producer starts from the original curve recipes.

```sh
cargo test -p viboceros --bin viboceros orient_curve
cargo test -p viboceros-command -p viboceros-oracle --lib orient_on_curve
python3 -m unittest tools.rhino_oracle.test_orient_on_curve
```

[Rhino command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/orientoncrv.htm)
describes the target behavior. Captures and comparisons are under
`tools/rhino_oracle/{fixtures,observations}/orient_on_curve*`.
