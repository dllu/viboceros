# Direction Analysis

[Analysis commands](analysis-index.md) · [Interactive Dir](direction.md)

`ShowDir` adds selected curves, surfaces, polysurfaces and meshes to a persistent
direction display, or asks for objects first. The Direction Analysis panel stays
open while modeling. `ShowDirOff` closes it and removes all analysis membership.

Direction, U and V checkboxes control curve tangents, oriented normals and surface
parameter axes. Direction color changes tangent/normal arrows; U/V use the axis
colors. The display shares `Dir`'s drawing code and mesh sampling limit. Arrow
pixels, density, native appearance preferences and dock layout are not matched.
Small trimmed regions use regular trim locations if the interior sampling grid
misses the face.

Add Objects and Remove Objects update membership using the current selection or
a new object pick. Repeating `ShowDir` adds to the existing display. Selection
changes alone do not remove analyzed objects. Hidden objects and hidden layers
keep their membership but draw no arrows. Delete removes their live display;
Undo restores it. Turning analysis off stays off through subsequent model Undo.

Flip Direction, Reverse U, Reverse V and Swap UV act on the current selection,
asking for objects when it is empty. These targets also join the analysis display.
Flip uses the ordinary `Flip` command's direction and solid policy. UV edits use
`Dir`'s trim-preserving single-face edits; polysurfaces and unsupported UV targets
are skipped. Explicit polysurface face batching remains available through
[`Dir Faces=...`](direction.md).

Accepted edits preserve source identities and create ordinary `Flip`/`Dir` Undo
entries. Flips and multiple-object UV edits retain preselection through Undo.
Single-object UV edits clear changed inputs on Undo and restore preselection on
Redo. Command-first edit picks are released on completion and replay. Display settings and membership
changes do not add history or clear Redo, and panel display controls can be used
without canceling an active point getter.

These console bindings expose panel actions in Viboceros; they are extensions to
Rhino's named `ShowDir` and `ShowDirOff` commands:

```text
ShowDir [Add|Remove]
ShowDir [Direction=Yes|No] [U=Yes|No] [V=Yes|No] [Color=r,g,b]
ShowDir Flip|UReverse|VReverse|SwapUV
```

## Validation

51 frozen Rhino 8 command/control recipes cover preselection, command-first
picking, checkbox changes, panel edit buttons, additive membership, new targets,
Hide/Show, Delete and Undo/Redo. Shared 3DM inputs cover rational and holed faces,
a sphere and a six-face box; line and quad-mesh definitions are identical across
engines. Public object analysis modes and owned native panel controls provide
display-state evidence. Timed public keystrokes answer owned selection getters.

Portable registry and application replays compare complete model definitions,
samples, selection, visibility, membership and display flags at absolute `1e-9`.
Raw responses and frozen producer bytes remain under
`tools/rhino_oracle/observations/`. This corpus does not establish general geometry,
rendered appearance, performance or saved-file analysis-state parity.

```sh
cargo test -p viboceros-command --lib direction::analysis
cargo test -p viboceros-oracle --test direction_analysis_native
cargo test -p viboceros --bin viboceros app::tests::direction_analysis
cargo test -p viboceros --bin viboceros viewport::direction
python3 -m unittest tools.rhino_oracle.test_direction_analysis
```

Reference: [Rhino Direction Analysis](https://docs.mcneel.com/rhino/8/help/en-us/commands/dir.htm).
