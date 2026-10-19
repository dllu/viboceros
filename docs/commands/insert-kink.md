# InsertKink

Select one curve or surface and run `InsertKink`, or start the command and pick
its source. Pick or type points repeatedly, then press Enter to commit one Undo
step. Esc discards every staged pick. Curve picks use closest points; a bare
number is not a curve parameter.

```text
InsertKink w2,1,0 w4,0,0
InsertKink Direction=Both Symmetrical=Yes w3,4,0
InsertKink SubCrv=object-id,start-parameter,end-parameter w2,1,0
```

Curves become PolyCurves containing exact trimmed pieces. Original domains and
closed-curve seams remain in place. Lines, arcs and polylines retain their leaf
classes. Repeated joint picks do not add extra pieces; an endpoint pick can wrap
an ordinary curve in a one-piece PolyCurve. Periodic NURBS pieces are clamped
when split. No fitting or shape approximation is introduced.

Surfaces receive degree-multiplicity knots. Direction describes the displayed
isocurve: U inserts constant-V knots, V inserts constant-U knots, and Both does
both. Symmetrical mirrors insertion around each parameter-domain midpoint.
Midpoints displays guide isocurves between existing knot lines. Toggle switches
U/V. Surface options are remembered by the command registry. Single-face B-reps
retain their vertices, edge curves, UV trims and orientation through exact
underlying-surface refinement. Picks resolve on that underlying surface even
inside trimmed holes. Polysurfaces and block members are excluded.

At the initial source prompt, `SubCrv` accepts a curve and two endpoint picks
without modifying it. Enter then finishes kink insertion on the shortened copy.
The scripted `SubCrv=id,start,end` form uses source parameters. The resulting
object has fresh attributes on the current layer; the original is retained.
An empty or cancelled temporary getter creates no object. This temporary getter
currently accepts endpoint points; its nested numeric-length/direction options
are not implemented.

Replacements retain object identity, attributes and groups. Curves and untrimmed
surface edits clear geometry user text; trimmed B-reps retain it. Preselection
remains selected; command-first results finish unselected through Undo/Redo.
Typed interactive points use the shared unit/calculator, relative-point, snap,
coordinate-filter and constraint input paths.

The retained 101 actual Rhino commands include polynomial/rational curves,
periodic curves, conics, analytic and composite pieces, repeated/end/joint picks,
custom domains, elevation, reversal, spatial placement, temporary subcurves,
rational surfaces, rectangular/curved holes, options, cancellation and rejected
polysurface input. API/UI replays compare definitions, samples, metadata and
history at `1e-8` epsilon. Curve and untrimmed-surface edits also have 3dm
roundtrip checks; the trimmed edits are checked in memory against native
source files and recorded topology. These cases do not establish arbitrary command
equivalence, continuous error bounds, or general performance. The two unused
outer NURBS knots are compared in the importer convention; all active knots are
checked. Surface B-reps use the portable exact kernel refinement in both builds;
standalone NURBS surfaces and curve trims also support the native backend.
Twenty-four additional replays compare standalone NURBS surfaces with the
captured native surface definitions.

[Validation receipts](../insert-kink-validation.json)

```sh
cargo test -p viboceros-command --test insert_kink
cargo test -p viboceros insert_kink
cargo test -p viboceros-geometry --lib surface_refinement_keeps_topology
python3 -m unittest tools.rhino_oracle.test_insert_kink_probe
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_insert_kink
```

Public reference: [McNeel point-edit commands](https://docs.mcneel.com/rhino/8/help/en-us/commands/removeknot.htm).
