# Starting transforms before selecting objects

[Transforms](transforms.md) · [Copy and history](transform-copy.md)

Enter `Move`, `Copy`, `Scale`, `Scale1D`, `Scale2D`, `Rotate`, `Rotate3D`,
`Mirror`, or `Shear` with no objects selected. Pick the source objects, then press Enter
to continue to the command's point prompts. Clicks and selection windows use
the normal group picking policy; `SelID object-id` selects one object directly.
`SelAll` and `SelNone` can adjust selection during this phase.

Copy choices and Mirror plane options become available after accepting the
sources. Move/Copy placement options also begin after source acceptance.
These options do not take effect at the source selection prompt. Enter with
no source ends the command. Escape while selecting clears the pending picks;
Escape after accepting sources retains them and makes no pending geometry edit.

## Ordering, selection, and history

Command-first sources follow pick order. A batch selection such as SelAll adds
objects in document order. Preselected sources continue to follow document
order. Repeated copies retain the accepted source order, even if the current
selection enumeration subsequently changes.

`Copy=Yes` retains source selection and leaves copies unselected. An accepted
in-place transform clears the picked source selection. An exact identity
in-place transform retains the picks and makes no history entry. Mirror's Object
plane choice has its own [source selection cleanup](mirror.md).

Undo removes command-first source selection, including when the originals were
unchanged by copying. The [Copy command](move-copy.md) also releases preselected
sources on Undo. Copied group definitions remain empty after Undo. Redo
preserves the existing transform replay policy, including output selection
captured by SelLast before Undo. Rejected geometry leaves the picked sources,
previous accepted copies, and history intact.

## Native checks

The input recipes were prescribed before capture. Rhino runs in private Xvfb
through `run_headless.sh`; the shared desktop receives no keyboard or mouse input.

| Inputs | Raw observations | Coverage |
| --- | --- | --- |
| [61 selection cases](../../tools/rhino_oracle/fixtures/transform_sources.json) | [Selection observations](../../tools/rhino_oracle/observations/transform_sources.json) | Seven commands, reverse-order SelID picks, partial groups, both Copy choices, cancellation before/after source acceptance, transparent selection commands, Mirror options |
| [12 identity cases](../../tools/rhino_oracle/fixtures/transform_sources_identity.json) | [Identity observations](../../tools/rhino_oracle/observations/transform_sources_identity.json) | Six commands, identity maps, both Copy choices, grouped sources |

The app replays the recipes and compares raw object order, geometry, identities,
attributes, selection, group membership, SelLast, Undo, and Redo at 1e-9 absolute
coordinate tolerance. Existing preselection fixtures run alongside these cases.
App tests also exercise mouse group picks, selection removal, real Escape,
restricted group peers, and atomic rollback when transformed coordinates overflow.
Native mouse/window ordering and command-first subobject transforms have not been
exhaustively measured. Additional [Move/Copy placement captures](move-copy.md)
cover their shared source selection and separate point workflows.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/transform_sources.json --scheme VibocerosOracleMirrorPreview --timeout 300
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/transform_sources_identity.json --scheme VibocerosOracleMirrorPreview --timeout 300
cargo test -p viboceros --bin viboceros transform_copy
python3 -m unittest tools.rhino_oracle.test_transform_copy
```
