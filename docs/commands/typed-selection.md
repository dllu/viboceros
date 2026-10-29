# Extrusion and block selection

| Command | Matches | Current selection |
| --- | --- | --- |
| `SelExtrusion` | Lightweight extrusion objects | Adds matches |
| `SelBlockInstance` | Top-level block instances | Adds matches |
| `SelMirroredBlocks` | Instances whose placement reverses orientation | Replaces selection |

All three commands accept no arguments, skip hidden/locked objects and layers,
and select matching objects directly without expanding groups. Geometry,
attributes, definitions, groups, layers and Undo/Redo remain unchanged.
`SelMirroredBlocks` also clears the current selection when nothing matches.

A shape-equivalent ordinary B-rep is not a lightweight extrusion. Block contents
are not searched recursively. Mirrored selection tests each top-level placement:
one or three negative scale axes reverse orientation; two preserve it. A nested
definition's inner reflection does not change its parent's placement classification.

Twenty-seven immutable Rhino command captures cover these rules, existing
selection, groups, object/layer restrictions, shear, nested references and empty
matches. Both Rust backends compare every selected index, source type, unchanged
source flag and Redo result. The Python oracle operation `typed_selection` uses
the fixed recipes in `fixtures/typed_selection*.json`; live captures require an
owned private settings scheme and a dedicated single-iteration batch.

```sh
cargo test --frozen -p viboceros-oracle --lib typed_selection
cargo test --frozen -p viboceros-oracle --features native-smlib --lib typed_selection
cargo test --frozen -p viboceros-command --lib geometry_selection
cargo test --frozen --bin viboceros typed_selection
python3 -m unittest tools.rhino_oracle.test_typed_selection
```

See [blocks](blocks.md), [object details](object-details.md), and Rhino's
[selection commands](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm).
