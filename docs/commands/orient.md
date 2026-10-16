# Orient and Orient3Pt

[Command reference](README.md) · [Transforms](transforms.md)

Start `Orient` with objects or grips selected, or select sources after starting
it and press Enter. Pick two reference points followed by two target points.
`Orient3Pt` (`Orient3Point`) takes three references and three targets. Typed
points use the shared world/CPlane, relative-point, unit, snapping, and point
filter input. The accepted references and targets remain visible as markers.

Objects preview from the first target prompt. The first target translates the
reference origin; the second determines direction and scaling. In `Orient3Pt`,
the third determines the remaining orientation. Preview and acceptance share one
solver and retain the original source IDs and grip indices.

`Scale=No` preserves dimensions. `Orient` also accepts `Scale=1D` along the
reference axis and `Scale=3D` for uniform scaling. `Orient3Pt Scale=Yes` uses the
first point-pair length ratio; its third point sets orientation only. Scale
initially defaults to No and is remembered separately for each command, including
changes accepted at prompts before cancellation. Copy follows
[RememberCopyOptions](remember-copy-options.md), initially No.

At any point prompt, edit `Copy=Yes|No` and `Scale=`. With `Copy=Yes`, continue
picking complete target pairs or triples for more placements from the original
sources. Escape keeps accepted copies and ends the command. `Orient3Pt` also
finishes on Enter. In `Orient`, Enter at the first target prompt uses the first
reference point as the target, including in a copy batch. All accepted placements
in a batch share one Undo step. Canceling before a placement makes no model edit.
Invalid coincident references or collinear triples keep the current prompt so
that another point can be picked.

Whole-object copies preserve source selection for preselection and release it
when the command's source getter was used. New objects remain unselected. A
single whole source allocates copied group definitions without assigning their
memberships; multiple sources use the existing shared transform group policy.
Grips use the shared object/grip transform path. Their copies retain the input
owner and create an edited owner, with the existing grip display and group rules.

Complete commands remain available:

```text
Orient 0,0,0 2,0,0 5,2,1 5,6,1 Scale=1D Copy=Yes
Orient3Pt 0,0,0 2,0,0 0,1,0 5,2,1 5,6,1 5,2,4 Scale=Yes
```

Reference-point coincidence uses numerical validation, independently of model
absolute tolerance. This accepts distinct reference pairs and triples whose
spacing is smaller than the model tolerance, as Rhino does. Opposite two-point
directions use the existing normal-frame constructor's OpenNURBS perpendicular
axis policy. Nearly opposite directions retain their nonzero cross-product axis.

The native fixtures use four noncoplanar witness points, actual Rhino commands,
preselection and command-first picking, spatial and opposite frames, CPlanes,
scale changes, short references/targets, copy batches, groups, target defaults,
and cancellation. UI tests independently replay the fixtures and check preview,
placements, selection, group counts and local Undo/Redo. Real pointer tests check
preview and accepted maps in all four default views. Separate fresh-scheme
captures isolate scale persistence after cancellation.

Rhino's `RunScript` returns true when `Orient3Pt` is canceled at its third target
in the recorded case, despite leaving geometry unchanged. Cancellation recipes
compare model state rather than that return flag. Completed recipes require a
successful native command. Group member order with multiple whole sources,
native grip workflows, native Undo/Redo and rendered appearance are not established
by these fixtures. Nearly collinear and extreme-coordinate frames are not
qualified. General performance and complete Rhino parity remain open.

[Validation and source fingerprints](../orient-points-validation.json) record the
actual comparison counts and executed checks. Discovery rows remain separate.
The local producer starts from the authored references and source witnesses;
it never consumes native-returned transforms or geometry.

```sh
cargo test -p viboceros --bin viboceros orient_points
cargo test -p viboceros-command --lib orient_
cargo test -p viboceros-oracle --lib orient_points
python3 -m unittest tools.rhino_oracle.test_orient_points
```

Public references: [Orient](https://docs.mcneel.com/rhino/8/help/en-us/commands/orient.htm)
and [Orient3Pt](https://docs.mcneel.com/rhino/8/help/en-us/commands/orient3pt.htm).
