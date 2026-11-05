# Block and extrusion selection

| Command | Selection behavior |
| --- | --- |
| `SelBlockInstance` | Adds selectable top-level block instances |
| `SelBlockInstanceNamed "Bolt *"` | Adds instances whose definition names match a case-insensitive `*` / `?` pattern |
| `SelMirroredBlocks` | Replaces selection with instances whose placement reverses orientation |
| `SelExtrusion` | Adds lightweight extrusion objects, including open extrusions |

Object and layer visibility/locking apply to all four commands. Type filters
select individual objects without expanding their groups. Name matching uses
block definition names; instance object names are independent. Quote names with
spaces. With no matches, the additive commands retain preselection;
`SelMirroredBlocks` clears it. `SelPrev` can restore the previous picks.

Mirroring is determined by the sign of the instance placement's linear
determinant. Two negative scale axes preserve orientation; one or three reverse
it. Nested members do not change the top-level instance classification. The
predicate uses the kernel's exact binary64 determinant sign, including scales
whose determinant would underflow or overflow in ordinary floating-point math.

`SelExtrusion` follows the retained [extrusion representation](../extrusions.md).
An ordinary surface or B-rep with the same shape does not qualify. These commands
preserve geometry, attributes, block definitions, group membership and model
history. They remain available through command input and completion.

## Qualification

Private Xvfb captures from licensed Rhino 8 cover 43 command sequences:
`type_selection_clean` (35) and `type_selection_empty` (8). They include reflected,
sheared and nested instances; Unicode and wildcard names; grouped peers;
preselection; restricted layers/objects; repeated commands; SelPrev and a pending
Point Redo. Native geometry CRCs include definition members. Backend, application
and release Python replays compare every stage, with unchanged shared geometry
and `1e-12` tolerance for placement coefficients.

An earlier 35-record `type_selection` capture remains immutable. Its first
command agrees with the clean capture; trailing Cancel commands released picks
before the next command. Its later selection states are retained as evidence of
that scripted sequence and are not used as repeated-selection references.

```sh
cargo test -p viboceros-command --lib block_selection
cargo test -p viboceros-oracle --test type_selection
cargo test -p viboceros --bin viboceros type_selection
python3 -m unittest tools.rhino_oracle.test_type_selection
python3 -m tools.rhino_oracle.type_selection_replay
```

Command semantics follow [Rhino's selection reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm).
Linked/reference-file block states and the native block-list dialog are not
implemented; provide the definition name or pattern on the command line.
