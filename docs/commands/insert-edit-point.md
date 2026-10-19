# InsertEditPoint

Run `InsertEditPoint`, select one curve, and pick locations on it. Edit-point
markers update in every viewport. Enter accepts all picks as one history item;
Escape or Cancel preserves the original curve and redo history.

This command preserves the curve's shape. It chooses a knot whose Greville
station corresponds to the projected pick, then uses exact knot refinement.
An existing edit point is not inserted twice. Closed and periodic curves,
rational weights, composite leaves and custom parameter domains are supported.
Some periodic picks near a seam have no valid insertion in the active domain
and leave the net unchanged. Picking the seam can clamp that end.

Enter accepts conversion to the NURBS editing representation even without a
pick. Accepted commands replace the object and clear redo, including unchanged
control nets. Object identity, attributes and groups survive; regeneration
clears geometry user text as in the native command.

Use `SubCrv` during source selection to work on a temporary portion. The shared
range getter supports start/end picks, length and direction controls. The
original curve remains, and Enter adds the refined portion.

For command-line or API batches:

```text
InsertEditPoint 1.374625,1.092,0
InsertEditPoint Points 1.374625,1.092,0 3.32,-0.2,0
InsertEditPoint Points SubCrv=curve-uuid,0.2,0.8 1.374625,1.092,0
```

The numeric SubCrv range uses source parameters. Live previews never replace
the source. Stale sources and invalid batches fail before mutation.

The [validation receipt](../insert-edit-point-validation.json) records 216
geometry/input workflows, 24 history witnesses and four independent empty-Undo
selection cases from Rhino 8.32.26160.13001. Curve definitions, representations,
weights, domains, sampled geometry, metadata, selection and Undo/Redo are
compared. Raw curve weights use a relative tolerance of `1e-12` in addition to
an absolute `1e-8`; coordinates and active knots retain the absolute checks.
Only the two omitted outer knot slots are adapted for the interchange format.

Pre-existing grip displays, exhaustive unclamped/repeated-knot layouts, all
mouse/high-DPI configurations and large-net performance remain unqualified.
The public SDK projection diagnostics document the numerical boundary behavior;
they do not identify Rhino's proprietary solver implementation.

```sh
cargo test -p viboceros-command --test insert_edit_point_command
cargo test -p viboceros --bin viboceros tests::insert_edit_point
cargo test -p viboceros-geometry --lib edit_point_insertion
```

Add `--features native-smlib` to run the command/API and application replays
through the native B-rep build. Saved replays do not launch Rhino.
