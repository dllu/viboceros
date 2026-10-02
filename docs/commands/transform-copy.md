# Repeated transform copies

[Transform commands](transforms.md) · [Copy preferences](remember-copy-options.md)

With objects selected, start `Scale`, `Scale1D`, `Scale2D`, `Rotate`,
`Rotate3D`, or `Shear`, then edit `Copy=Yes` at the point prompt. Each accepted
target copies the original selection. Enter or Escape finishes the command and
keeps accepted copies. One external Undo removes the batch; Redo restores it.
Typing `Undo` during these prompts leaves accepted copies intact.

| Input form | Repeated input after the first copy |
| --- | --- |
| Scale / Scale2D, numeric factor | Another factor, or a first reference point |
| Scale / Scale1D / Scale2D, reference points | Another target; the first reference stays fixed |
| Scale1D, numeric factor | Another direction; the factor stays fixed |
| Rotate / Rotate3D, numeric angle | Another angle, or a first reference point |
| Rotate / Rotate3D, reference points | Another target; the first reference stays fixed |
| Shear, reference direction | Another numeric angle or target point |

For numeric Scale1D, a number entered after its first copy is a point distance
constraint for the next direction. It does not change the factor or create a
copy by itself. Rotate3D exposes Copy only after both axis endpoints are chosen.
Mirror supports Copy editing but finishes after one mirrored result.

`Copy=No` makes the next accepted target transform the originals in place and
finish the command. Earlier copies remain. The saved Copy preference follows
the last successful edit; toggling Copy and finishing without another edit
does not replace that preference. A rejected target leaves earlier edits and
the active prompt intact.

Originals remain selected and copies are unselected. Copies inherit object
names, color sources, colors, and layers. Multiple selected sources recreate
their group memberships for each copy. Undo retains those new group definitions
empty. An in-place edit renews the originals' object order. SelLast recalls all
accepted copies in the batch.

## Verification and limits

The [49 independent input recipes](../../tools/rhino_oracle/fixtures/transform_copy.json)
are generated before measurement. The [raw Rhino 8.32 observations](../../tools/rhino_oracle/observations/transform_copy.json)
retain command histories and terminal events, object order and source identity,
selection, attributes, group membership, SelLast, and external Undo/Redo.
The application test feeds the prescribed inputs through its command prompt;
geometry and complete document snapshots match at absolute epsilon `1e-9`,
relative zero. Group names are retained as raw telemetry; their automatic
numbering across separate documents is not compared.

Rhino's numeric Rotate/Rotate3D repetition ends with `Nothing` on Enter and
`Cancel` on Escape despite committing accepted copies. Those raw results are
preserved. Our registry returns success for each accepted geometry edit.

These cases use WorldXY and point witnesses. They verify interactive affine
maps and document behavior; they do not cover every curve, surface, or mesh
representation. Existing [plane transform tests](../plane-transforms.md)
separately cover other CPlanes. This capture does not measure viewport mouse
picks, automatic bounding-box centers, default factors/angles accepted with
Enter before the first edit, general calculator expressions, or every invalid
input. Those workflows remain incomplete. ScaleNU and orientation prompts do
not yet support this repeated target session. Shared Copy preferences are
currently session-only.

These history policies currently apply to interactive sessions. Full argument
registry invocations do not yet retain new group definitions on Undo or renew
in-place object order.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/transform_copy.json --scheme VibocerosOracleTransformFresh --timeout 300
cargo test -p viboceros transform_copy --bin viboceros
cargo test -p viboceros-document history_group --lib
cargo test -p viboceros-command copy_options --lib
python3 -m unittest tools.rhino_oracle.test_transform_copy
```

Use a fresh private scheme beginning with `VibocerosOracle`. Capture requires a
dedicated Xvfb display, validates bounded numeric inputs before launch, runs
outside RunPythonScript's undo record, and cleans up owned objects and groups.
It is an interactive app replay fixture; the standalone geometry `replay` CLI
does not accept this operation.
