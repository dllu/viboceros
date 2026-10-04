# Transform copies and history

[Transform commands](transforms.md) · [Copy preferences](remember-copy-options.md) ·
[Scalar defaults](transform-defaults.md)

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

At the initial base-point prompt of Scale, Scale1D, or Scale2D, Enter uses
the combined World bounding-box center of the selected geometry. Rotating,
tilting, or translating the active CPlane does not change that base point.
Scale2D still scales along the active CPlane's axes.

`Copy=No` makes the next accepted target transform the originals in place and
finish the command. Earlier copies remain. The saved Copy preference follows
the last successful edit; toggling Copy and finishing without another edit
does not replace that preference. A rejected target leaves earlier edits and
the active prompt intact.

Preselected originals remain selected and copies are unselected. Command-first
selection has distinct [cleanup and Undo rules](transform-sources.md). Copies inherit object
names, color sources, colors, and layers. Multiple selected sources recreate
their group memberships for each copy. A single selected source produces an
ungrouped copy and corresponding empty group definitions, including when its
group has unselected peers. Undo retains new group definitions empty.

These seven affine commands enumerate preselected inputs in document object order.
Command-first sources follow pick order. An in-place edit renews all selected
originals after untouched objects, including points fixed by a nonidentity
map. An exact identity map skips in-place replacement; Copy=Yes still creates
copies. SelLast recalls all accepted copies in a repeated batch. These history
policies also apply to full argument registry invocations of the seven commands.

Rotate and Rotate3D clean up angles close to cardinal turns using an attributed
Rust adaptation of public
[OpenNURBS rotation policy](https://github.com/mcneel/opennurbs/blob/8.x/opennurbs_xform.cpp).
The measured ±360, 720, and positive angles through `9e-7` degrees produce
identity maps; `1.2e-6` degrees remains a rotation. Scale factors only `1e-14`
above one still renew objects. Scale2D factor one is an exact
identity on tilted CPlanes. The general geometry kernel preserves tiny
rotations without the command's cutoff.

## Verification and limits

Input recipes are generated before measurement; raw Rhino 8.32 observations
retain command histories and terminal events, object order and source identity,
selection, attributes, group membership, SelLast, and external Undo/Redo.

| Recipes | Raw observations | Application replay |
| --- | --- | --- |
| [49 repeated sessions](../../tools/rhino_oracle/fixtures/transform_copy.json) | [Repeated sessions](../../tools/rhino_oracle/observations/transform_copy.json) | Command prompt |
| [56 single edits](../../tools/rhino_oracle/fixtures/transform_copy_script.json) | [Single edits](../../tools/rhino_oracle/observations/transform_copy_script.json) | Command prompt and full registry invocation |
| [18 automatic centers](../../tools/rhino_oracle/fixtures/transform_copy_center.json) | [Automatic centers](../../tools/rhino_oracle/observations/transform_copy_center.json) | Command prompt with WorldXY, rotated, and tilted CPlanes |
| [64 exact and near identity inputs](../../tools/rhino_oracle/fixtures/transform_copy_identity.json) | [Identity boundaries](../../tools/rhino_oracle/observations/transform_copy_identity.json) | Command prompt and full registry invocation |

The application tests feed the prescribed inputs through the actual command paths;
geometry and complete document snapshots match at absolute epsilon `1e-9`,
relative zero. Group names are retained as raw telemetry; their automatic
numbering across separate documents is not compared. The source construction
record is preserved, so SelLast for a transform that performs no edit is tested
against the preceding setup record.

Rhino's numeric Rotate/Rotate3D repetition ends with `Nothing` on Enter and
`Cancel` on Escape despite committing accepted copies. Those raw results are
preserved. Our registry returns success for each accepted geometry edit.

These cases use point witnesses. They verify affine maps and document behavior;
they do not cover every curve, surface, or mesh representation. Automatic
centers use the kernel's tight World bounds for supported geometry; native
center comparisons here cover points only. Existing
[plane transform tests](../plane-transforms.md) cover additional CPlane geometry.
The separate [scalar-default session](transform-defaults.md) verifies Enter
before the first edit. This capture does not measure viewport mouse picks,
general calculator expressions, or every invalid input. Those workflows remain
incomplete. [ScaleNU](scale-nu.md) has a separate repeated per-axis workflow. Orientation
prompts do not yet support this repeated target session, and other transform
commands retain their separate history policies. Shared Copy preferences are
currently session-only.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/transform_copy.json --scheme VibocerosOracleTransformFresh --timeout 300
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/transform_copy_script.json --scheme VibocerosOracleTransformScriptFresh --timeout 300
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/transform_copy_center.json --scheme VibocerosOracleTransformCenterFresh --timeout 300
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/transform_copy_identity.json --scheme VibocerosOracleTransformIdentityFresh --timeout 300
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
