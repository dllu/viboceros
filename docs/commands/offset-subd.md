# OffsetSubD

[Meshes and SubDs](meshes-index.md) · [Command input](../command-line.md)

`OffsetSubD` copies editable SubDs by moving their control vertices. Start it
with SubDs selected, or select inputs at the command prompt and press Enter.
Enter a distance and options to update the control-cage preview, then Enter to
commit. Click an input to reverse its individual side; `FlipAll` reverses every
input. Escape cancels without consuming pending Redo.

```text
OffsetSubD
OffsetSubD 0.75 Solid=Yes DeleteInput=No
OffsetSubD Distance=-0.75 BothSides=Yes
OffsetSubD 0.75 BothSides=Yes Solid=Yes FlipAll
```

A complete typed invocation with a distance executes immediately after source
selection. Without a distance, the application opens its options phase. Defaults
are distance 1, Solid=No, BothSides=No and DeleteInput=No. These choices are
remembered outside model history, including after cancellation. FlipAll and
individual side flips start afresh on each invocation.

Controls move along the normalized sum of their distinct limit-sector normals.
This agrees with the measured SDK outputs for smooth boundaries, hard creases,
warped faces and extraordinary vertices. It moves the control net; the resulting
smooth surface is not a certified constant-distance offset everywhere.

Solid adds an oppositely oriented input skin and quad walls along naked edges,
retaining boundary creases. Closed inputs return the offset skin alone. Negative
solid distances reverse the open shell's orientation. BothSides without Solid
creates positive and negative skins as separate objects. BothSides with Solid
first offsets positively, then offsets that result by twice the distance in the
opposite direction, using its re-evaluated normals. Curved cages therefore need
not be symmetric about the original. Closed inputs follow the same two steps.

Outputs retain source attributes, layers and ordered groups, and start
unselected. Kept inputs remain selected at command completion. Sources and
results follow document order, with both skins adjacent per source. DeleteInput
removes the originals. One Undo/Redo step covers the edit; a deleted preselected
input returns selected on Undo, while a command-first input returns unselected.
All geometry is staged before editing. Changed sources, selection, attributes,
groups or tolerance reject a pending preview.

The frozen captures cover 48 public SDK cases, 52 single-source command cases,
eight multi-source cases and a seven-command preference/cancellation sequence.
Native 3DM source archives preserve raw component order. Replays compare controls,
raw face/edge topology, tags and vertex limit stations at `1e-10`, plus IDs,
attributes, groups, selection and actual Undo/Redo. Native macro cleanup releases
retained picks after EndCommand; application tests compare completion first and
explicitly model that separate cleanup before history replay. Local pointer tests
exercise source flipping and the preview in wireframe, shaded and ghosted modes.
Refined nets and asymmetric soft sharpness also undergo native 3DM roundtrips.

Inputs and outputs are bounded to 100,000 controls/faces, 400,000 edges/corners
and 64 MiB per archive; aggregate staged archives also have a 64 MiB bound. Each kernel offset
admits at most 16 million summed squared vertex face counts. Edges
must have one or two incident faces, and normals must be defined. Mixed geometry,
self-intersection repair, symmetry, native face packing, texture/material parity,
comparative speed and physical native mouse gestures need further qualification.

```sh
cargo test -p viboceros-geometry --lib subd_offset
cargo test -p viboceros-command --lib offset_subd
cargo test -p viboceros-oracle --test subd_offset
cargo test -p viboceros --bin viboceros offset_subd
python3 -m unittest tools.rhino_oracle.test_subd_offset
```

References: [OffsetSubD](https://docs.mcneel.com/rhino/8mac/help/en-us/commands/offsetsubd.htm),
[public SDK](https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.subd/?member=offset-system.double-system.boolean).
