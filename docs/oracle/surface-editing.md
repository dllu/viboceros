# Surface and edge editing comparisons

[Oracle overview](../oracle.md) · [Evidence index](README.md)

## Surface trim removal

[UntrimAll](../commands/untrim-all.md) has 100 saved private-Xvfb command cases in
[the fixture](../../tools/rhino_oracle/fixtures/untrim_all.json) and
[Rhino capture](../../tools/rhino_oracle/observations/untrim_all.json). The probe
records public command completion, full constructed and inserted geometry,
result topology and definitions, chronological object order, selection, and
attributes. Source recipes are regenerated without using observed targets.
For raw surfaces, document snapshots compare a natural B-rep wrapper; the
constructed record retains the original NURBS surface definition.

The Python replay compares all 96 non-box cases with absolute epsilon `1e-9`
and relative epsilon zero, including four rejected two-face selections and
both workflows. Four independently built
box cases compare rejection, attributes, and unchanged geometry within each
engine; their different face/edge tables are not a topology-parity claim.
Command events and history are retained as diagnostics, outside the geometric
comparison. The adapter rejects changed geometry in those box cases before
comparison and sends only the original source fixture to the native engine.
Native commands and replay are untimed.
The recorded replay's maximum absolute difference is `3.55e-15`.

```sh
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_all.json tools/rhino_oracle/observations/untrim_all.json
```

[UntrimBorder](../commands/untrim-border.md) has another 100 cases in
[its source fixture](../../tools/rhino_oracle/fixtures/untrim_border.json) and
[Rhino capture](../../tools/rhino_oracle/observations/untrim_border.json). The same
replay compares 96 complete definitions and scopes the four independent box
factories to rejection and unchanged geometry. Inner loops retain their exact
model-space curves, UV trims, tolerances, and winding while unused outer
topology is removed. The cases include multiple holes, reversed faces, shifted
and negative domains, natural surfaces, and exterior trims. Both commands
restore native parameter intervals on new boundary edges. Kernel checks also
exercise a hole on a closed cylinder with shared seam topology and permuted
multi-edge hole boundaries.

```sh
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_border.json tools/rhino_oracle/observations/untrim_border.json
```

The command reference lists the remaining face-subobject, hatch, and crease
settings gaps. These captures do not certify those workflows.

## Interior-hole removal geometry API

[`brep_remove_holes`](../geometry/remove-holes.md) records the public
`Brep.RemoveHoles` overloads on identical, owned 3dm source files. Its
[28 source recipes](../../tools/rhino_oracle/fixtures/brep_remove_holes.json) and
[Rhino capture](../../tools/rhino_oracle/observations/brep_remove_holes.json) compare
complete before/after topology, NURBS definitions, UV trims, and tolerances with
absolute epsilon `1e-9` and relative epsilon zero. Selecting one opening of a
joined through hole removes its wall and closes both openings. Disconnected
caps are edited independently. Empty and outer-only selections return no result;
the all-hole overload copies no-hole sources unchanged. This untimed API probe
does not establish interactive `UntrimHoles` parity. The
[component command](../commands/untrim-holes.md) now matches 110 native
cases: 44 face/edge cases, eight internal Undo/repicking cases, and 42 split-edge
perimeter-limit cases, including exact retained
curves and wall B-reps, source identity and metadata, current-layer defaults,
and edits surviving Esc, plus 16 multiple-pick and external Undo/Redo cases.
Local Undo discards the last pick; external Undo/Redo restores the complete
command. Multiple distinct preselected components fail without edits. The
viewport now supports edge/face postselection, options, ambiguity, and grouped
history; component preselection and window selection remain pending.
Raw command and Undo/Redo events/history are retained as diagnostics; only those
fields are omitted from the complete geometry/metadata comparison, including
the geometry snapshots after external Undo and Redo. These probes are untimed.
The release-mode Python replay matches all 110 cases with maximum absolute
error zero, including validation of each independently constructed 3dm source.

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/brep_remove_holes.json --observations tools/rhino_oracle/observations/brep_remove_holes.json --absolute-epsilon 1e-9 --relative-epsilon 0 --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_components.json tools/rhino_oracle/observations/untrim_holes_components.json --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_undo.json tools/rhino_oracle/observations/untrim_holes_undo.json --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_history.json tools/rhino_oracle/observations/untrim_holes_history.json --timeout 300
```

## Picked boundary restoration

General [Untrim](../commands/untrim.md) has 231 saved private-Xvfb cases:
71 preselection/mouse cases, six partial exterior variants, four opposite tube
opening picks, 24 local Undo/Enter/Escape sequences, 96 rectangular source
permutations, four split-edge variants and eight polynomial/rational surface
partial edits, 16 ignored multi-face exterior picks, and two positive controls.
The multi-face sources include explicit exact edge joins and both face orders.
All sources are exported independently before Rhino starts. Public picking
queries confirm the owned object and first target edge before real mouse input;
correctly targeted edges may still be ignored by the command.
The driver gates inputs on public command
lifecycle events and records complete per-click and per-Undo geometry.

Replay retains control nets, knots, scalar intervals, face sense, vertex records,
UV trims, metadata and the complete incidence graph, including native edge-table
order and numeric component indices. The rectangular allocation policy stores
only measured component ordering classes; geometry is constructed independently.
No edge permutation is applied during comparison. Other partial-loop allocation
classes remain unverified, as detailed in the command's coverage notes.
Captures are split into bounded owned jobs, and each finished batch is saved
before the next begins.

```sh
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_components.json tools/rhino_oracle/observations/untrim_components.json
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_history.json tools/rhino_oracle/observations/untrim_history.json
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_ordering.json tools/rhino_oracle/observations/untrim_ordering.json
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_multiface.json tools/rhino_oracle/observations/untrim_multiface.json
```

## Joined edge separation

`brep_unjoin_edges` compares the public geometry API on independently built,
identically exported sources. `unjoin_edge_command` compares actual command
preselection and owned mouse batches, including genuine external Undo/Redo.
The saved corpora contain 26 API cases, 48 preselection/mouse command cases,
and [42 sequenced selection cases](../../tools/rhino_oracle/fixtures/unjoin_edge_selection.json).
The latter mix modifier clicks and rectangles with `None` and prompt `Undo`;
each accepted input records the native SDK component selection. Complete
edge/vertex tables, surface control nets, UV trims, tolerances, object metadata, source
identities, and transient component selection remain in the comparison.
Only command lifecycle events and diagnostic history are excluded.

The command's copied-edge order differs from the API's input order. Captures
measure preselected batch sizes through twelve and two-/three-edge mouse
orders, including split shared edges and multiple sources. See
[command behavior and remaining limits](../commands/unjoin-edge.md) and the
[geometry kernel](../geometry/unjoin-edges.md).

The capture helper exports and validates all independent source responses
before starting Rhino. Owned paths replace caller paths and are cleaned after
the job. Mouse batches use one command pause, then an owned Enter/Escape event
to finish the multi-selection prompt. Sequence markers include a bounded step
index so repeated identical gestures are delivered independently. Inputs are
acknowledged before taking mouse/Undo selection snapshots; terminal `None`
is observed after command exit. Snapshots never drive input. The whitelist
permits modifier gestures and `None`/`Undo` keys only,
and owned key/button releases run even when an earlier release fails.
The independent rectangle adapter rejects tilted or nonlinear sources.
All live captures use private Xvfb.

```sh
python3 -m unittest tools.rhino_oracle.test_unjoin_edges
python3 -m tools.rhino_oracle.unjoin_edges_replay tools/rhino_oracle/fixtures/brep_unjoin_edges.json tools/rhino_oracle/observations/brep_unjoin_edges.json
python3 -m tools.rhino_oracle.unjoin_edges_replay tools/rhino_oracle/fixtures/unjoin_edge_command.json tools/rhino_oracle/observations/unjoin_edge_command.json
python3 -m tools.rhino_oracle.unjoin_edges_replay tools/rhino_oracle/fixtures/unjoin_edge_selection.json tools/rhino_oracle/observations/unjoin_edge_selection.json
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.unjoin_edges_capture tools/rhino_oracle/fixtures/unjoin_edge_command.json --timeout 300
```

## Shrink trimmed surfaces

Both [shrink commands](../commands/shrink-trimmed-surfaces.md) have 132 independent
private-Xvfb cases: 60 base, 28 history/order, and 44 additional geometry cases.
Sources use the shared B-rep recipe and are exported/validated before Rhino
reads owned 3DM artifacts. Captures record complete raw definitions and topology,
attributes, groups, selection, command events, history, Undo and Redo.

`shrink_trimmed_cases.py` regenerates each source-only matrix;
`shrink_trimmed_capture.py` bounds each owned Rhino process to eight cases.
`shrink_trimmed_replay.py` compares all geometry and numeric indices at `1e-9`
absolute epsilon and zero relative epsilon. Snapshot order alone is matched by
source identity: repeated identical inputs with fresh UUIDs demonstrate varying
native renewal order. Raw observations retain chronological order. The geometry
matrix covers rational rotated bounds, signed weights, exact multi-span crops,
joined topology, seams, singularities, and UV origins at `1e12`.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.shrink_trimmed_capture tools/rhino_oracle/fixtures/shrink_trimmed_surfaces.json --timeout 300
python3 -m tools.rhino_oracle.shrink_trimmed_replay tools/rhino_oracle/fixtures/shrink_trimmed_surfaces.json tools/rhino_oracle/observations/shrink_trimmed_surfaces.json
```

Face subobjects and disabling individual shrink sides remain outside this
coverage. These captures establish the stated cases, not full Rhino parity.
