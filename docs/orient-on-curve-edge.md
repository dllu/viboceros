# OrientOnCrv surface-edge guides

[Command usage](commands/orient-on-curve.md)

Pick a B-rep edge or a NURBS surface boundary at the `OrientOnCrv` guide prompt.
The existing component picker handles overlapping candidates. `SubCrv` accepts
these edges too, with the same directed endpoints, numeric length, direction
locking, and read-only endpoint preview used for curve guides. Complete commands
accept `Edge=<parent-id>,<edge-index>` and optional `SubCrv=start,end`; the
parameters belong to that edge's curve and indices belong to the current parent
topology.

The shared curve-reference resolver borrows B-rep edge curves or extracts an
owned surface boundary. The controller captures the parent geometry snapshot
and model tolerance. It discards the private guide and preview if that geometry
changes, the parent becomes unselectable or disappears, or tolerance changes.
It then asks for a new guide. Parent geometry and attributes remain intact;
only accepted placements enter document history. Repeated placements share
one Undo step. Intervening external edits retain their own history.

A full edge guide keeps source selection, whereas nested SubCrv clears it.
Both paths retain the captured source IDs for subsequent placements. No extracted
edge or temporary portion is inserted into the model.

The native harness runs the public command in a newly owned private Rhino
window. Compiled CLR expression delegates deliver bounded point/option input;
the host clicks the requested physical edge. This avoids Python callback
re-entry while Rhino's getters are running. The driver records its prompts and
sent inputs and repeatedly cancels on its deadline. An incomplete driver cannot
qualify a comparison.

The source-only fixtures include boxes, a curved NURBS surface boundary, and
cylinder edges. Cylinder constructors have different edge orderings and cap
topology. Those cases use an authored world-space `edge_anchor` to identify the
same physical guide independently in each implementation; the resulting native
index is diagnostic. The capture also exposed an incorrectly directed upper
cylinder rim; the constructor now reverses that edge and updates both incident
trim directions, retaining the same closed solid. The local producer never uses native-returned curve
geometry or frames as input. `Edge=` in product commands continues to use local
component indices.

Early captures are retained as setup diagnostics. They include Python callback
timeouts, unavailable CodeDOM, a non-writable expression node, cleared SDK
preselection, a malformed surface recipe, and a comparison that incorrectly
assumed identical cylinder edge numbering. A canceled copy command returned
false even though it kept the placed copies; the final driver ends copy input
with Enter. These discovery rows are excluded from qualification counts.

All 19 final edge placements match Rhino within `1e-6`, including source/copy
selection and unchanged parent geometry. Together with the earlier whole and
partial curves, the command comparison set has 100 cases. The UI independently
replays the 19 edge workflows and checks preview, placement, selection, and local
Undo/Redo. Local edge tests also cover typed input, ambiguous picks, stale or
unavailable parents, tolerance changes, numeric direction locking, and relative
copy anchors.

The final comparison and executed checks are recorded in
[the validation record](orient-on-curve-edge-validation.json). Native reference
point rotation, native undo/redo, using the guide parent among the sources,
all singular or self-intersecting inputs, general performance, and rendered
appearance remain unqualified. Numeric-length and relative-followup interaction
specifically on edges have local shared-path coverage; this native edge driver
currently accepts point endpoints and one placement per command.

```sh
cargo test -p viboceros --bin viboceros orient_curve
cargo test -p viboceros-command -p viboceros-oracle --lib orient_on_curve
cargo test -p viboceros-oracle --features native-smlib --lib orient_on_curve
python3 -m unittest tools.rhino_oracle.test_orient_on_curve
```
