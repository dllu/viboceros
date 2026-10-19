# InsertKnot

Run `InsertKnot` with one curve or surface selected, or start it and pick a
source. Pick points repeatedly or type curve parameters. Enter accepts all edits
as one document Undo step; Esc discards them. Document geometry stays unchanged while
the getter is active.

- `Automatic` inserts one knot halfway through every existing nonempty knot
  span. Each use is one staged action, including when it edits both surface
  directions or every polycurve segment.
- `Undo` reverses the last staged action. It keeps the command active and
  preserves the document's existing Undo/Redo history.
- `Symmetrical=Yes` mirrors manual locations across the active domain.
  Automatic bisects the existing spans independently of this setting.
- `Midpoints=Yes` shows parameter-midpoint markers or isocurves as guides.
  It does not force arbitrary point picks onto those midpoints.
- Surface `Direction=U|V|Both` names the displayed isocurve: U inserts V knots,
  V inserts U knots, and Both inserts both. `Toggle` switches U/V.

Point picks project onto the curve or underlying surface. Trim holes keep their
boundaries and do not restrict that projection. Knots have target multiplicity
one, so repeated picks do not keep increasing multiplicity. Lines and arcs
become exact NURBS; polycurves retain their segments with NURBS leaves and
global parameter charts. No fitting changes the source locus.

`SubCrv` at the source prompt selects a temporary portion. It shares endpoint,
numeric-length, unit, direction-lock and preview input with InsertKink. The
accepted output is a new object with default attributes on the current layer;
the original remains unchanged. Enter/Esc within temporary input returns to the
parent source getter. Polysurfaces and block members are excluded.

For a complete scripted edit, use `Points` to distinguish the repeated getter
syntax from the existing explicit knot-parameter API:

```text
InsertKnot Points w2,1,0 w4,0,0 Undo Automatic
InsertKnot Points Direction=Both Symmetrical=Yes w3,4,0
InsertKnot Points SubCrv=object-id,start,end Automatic
```

The older `InsertKnot parameter|u,v Multiplicity=n Direction=U|V|Both` form
retains its explicit parameter-axis meaning and target-multiplicity support.
See [NURBS scripting](nurbs.md). Interactive preferences belong to InsertKnot
and are independent of InsertKink preferences.

Both builds use the shared exact refinement policy. `native-smlib` delegates
curve, standalone-surface and single-face B-rep insertion to usd-brep. B-reps
retain vertices, edges and UV trims without refitting unchanged boundaries.
Source identity, object attributes and groups survive replacement. Curve and
untrimmed-surface edits clear geometry user text; trimmed B-reps retain it.

The [validation receipt](../insert-knot-validation.json) retains 224 actual
Rhino commands, their source 3dm files and complete outcomes. API and UI replay
compare curve/surface definitions, samples, geometry user text, selection and
Undo/Redo at `1e-8` in both backends. Thirty standalone-surface replays exercise
the same captured definitions. Independent tests cover guide positions, input
filters, numeric errors, local Undo, option ownership and redo retention. The
existing InsertKink captures remain regressions for the shared controller.
Symmetric pairs and central picks include a regression where reflecting a
projected rational midpoint previously inserted two adjacent knots. Implicit
mirrored stations now reuse a geometrically coincident existing knot.

The captures cover endpoint-defined temporary portions; numeric shortening
inherits the shared input policy but has no new InsertKnot-specific native
capture. Native metadata-only Undo/Redo was not captured, and Rust's command
result does not expose Rhino's complete Cancel/Failure enum. Edited 3dm
roundtrips, complete preference workflows, status-panel/pixel parity and general
performance remain unqualified. Display guides are bounded; the recorded
samples do not certify a continuous error bound.

```sh
cargo test -p viboceros-command --test insert_knot_command
cargo test -p viboceros app::tests::insert_
cargo test -p viboceros --features native-smlib app::tests::insert_
python3 -m unittest tools.rhino_oracle.test_insert_knot_command
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_insert_knot_command
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_insert_knot_followup
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_insert_knot_symmetry
```

Public reference: [McNeel point-edit commands](https://docs.mcneel.com/rhino/8/help/en-us/commands/removeknot.htm).
