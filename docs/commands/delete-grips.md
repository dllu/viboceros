# Delete selected grips

Run `PointsOn` or `EditPtOn`, select visible grips, and run `Delete`. The Delete
and Backspace keys also work when the viewport has keyboard focus. Text entry
keeps its usual editing behavior.

Curve control picks remove controls and rebuild the reduced knot vector. Edit
point picks use the corresponding curve controls; deleting an open endpoint
places the new endpoint at the first surviving original edit station. Closed
and periodic curves share their seam grips and retain at least three unique
controls. A request leaving only one grip preserves the curve. Selecting every
displayed grip deletes its owner.

Surface picks remove complete U or V rows, including the union of rows in both
directions. Incomplete rows preserve the surface. Use `SelU` or `SelV` to select
a row. Surface weights remain attached to their surviving controls.

Mesh vertex picks dissolve their incident face sets through OpenNURBS, retaining
the resulting facets, n-gons and vertex colors. The operation can leave a picked
vertex in place while combining its surrounding faces. Exact duplicate
vertices are welded in source order during reconstruction.

Selected grips take precedence over their selected parent object. Other
selected objects are deleted in the same Undo step. Hidden logical picks keep
their parent out of whole-object deletion but are not physically removed.
Object identities, names, colors and group memberships survive reconstruction.

`Undo` restores geometry and object order. Curve grip selection returns with the
original display; partial mesh edits restore an unselected display. `Redo`
replays the deletion. Running `PointsOff` after deletion keeps the display
closed on Redo; Undo restores the original display.
Edits that change neither geometry nor the native replacement state retain the
existing redo branch.

The saved Rhino 8.32.26160.13001 corpus contains 397 Delete/Undo/Redo workflows
and 1,193 command completions. Comparisons include full control nets, weights,
represented knots, domains, facets, n-gons, colors, grip flags, object order,
identity and selection. The protocol encodes the two omitted OpenNURBS outer
knot slots by repeating their neighbors; mesh color alpha is encoded as opacity.
Those representation conversions do not alter active geometry.

Nonlinear-to-nonlinear composite junction deletion, interior two-control
composite leaves, arbitrary closed composite seams, and trimmed surface edits
that change the trim parameter domain remain unsupported. Additional nonuniform
clamped seam layouts remain unqualified. Large-net performance
has not been benchmarked against Rhino.

Replay the saved public observations without launching Rhino:

```sh
cargo test -p viboceros-geometry --lib grip_deletion_tests
cargo test -p viboceros-oracle --lib complete_native_delete_grips_and_history
cargo test -p viboceros complete_native_delete_grips_application_and_history
cargo build --release -p viboceros-oracle
python3 -m tools.rhino_oracle.replay_delete_grips --output /tmp/delete-grips.json
```

Recapture with the existing licensed Rhino launcher in an isolated document:

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_delete_grips
```

[Validation and source hashes](../delete-grips-validation.json)
