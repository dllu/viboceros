# Sequential SubD edits and selection history

MergeFaces followed by global or local SubDivide preserves native vertex, edge
and face IDs, control topology, tags and evaluated geometry. Public SDK duplicate
and 3DM readback paths produce the same allocating results. Archive normalization
of legacy maximum-ID fields therefore needs no codec change for these paths.
Other allocating operations still need separate comparison.

The application now follows completed-command selection observed in Rhino:

- SubDivide clears component picks before returning to the command line. Partial
  children can be selected during EndCommand; those transient picks describe the
  mapped output faces but do not remain selected. Repeated partial refinement
  requires an explicit new selection.
- MergeFaces Redo restores original result faces only while their automatic
  selection has never been cleared. Picking other components, or clearing and
  reselecting even the same face, does not create automatic result eligibility.
- Whole-object picks made on a merge output before Undo clear on Undo and return
  on Redo. Unrelated picks survive. A later no-op command adds no Undo entry and
  does not disrupt this replay.

Component replay uses stable history entry IDs and geometry storage witnesses.
Explicit deselection removes eligibility only from the matching result version;
history replacement and stale-pick pruning do not count as user deselection.
Document history captures whole-object output picks before undoing geometry,
then restores them after redo. Repeated no-op cleanup coalesces before this final
capture marker. Snapshots share geometry storage rather than copying control nets.

## Evidence and limits

The [validation receipt](subd-edit-history-validation.json) binds current source
bodies and six immutable Rhino capture families, containing 68 workflows and 188
native 3DM models:

- 52 allocating workflows cover eight source nets, global/local refinement, SDK
  duplication and archive roundtrips. They retain the original cleanup-macro and
  trailing-Enter captures as geometry diagnostics.
- 20 of those workflows test completed-command selection using bare RunScript
  commands (16) and queued RhinoApp.SendKeystrokes commands (4). Both transports
  distinguish callback selection from command-line return.
- 16 selection-history workflows cover keeping, clearing and changing face,
  edge, vertex and whole-object picks, including clearing and reselecting the
  original result face on a second Undo/Redo cycle.

The IO audit compares active IDs, tags, loops and discrete properties exactly,
and control/limit points plus oriented sampled surface facets within `1e-7`.
Application replays compare completed selections and both Undo/Redo steps, then
check result geometry against the native archive. Existing MergeFaces, SubDivide,
grip, crease and extraction regressions cover shared selection behavior. The
same allocation corpus runs after explicit SMLib initialization. This does not
qualify every SubD allocation operation, material merge policy, custom face
textures, nonmanifold regions or comparative performance.

```sh
cargo test -p viboceros-document --lib selection::replay_tests
cargo test -p viboceros-io --test subd_face_merge
cargo test -p viboceros-io --test subd_face_merge --features native-smlib
cargo test -p viboceros --bin viboceros app::merge_faces::
cargo test -p viboceros --bin viboceros app::subdivide::
python3 -m unittest tools.rhino_oracle.test_subd_edit_history
```

The older [SubDivide measurement](../subdivide-qualification.json) remains a
historical geometry/getter record. Its callback children are checked as mapped
output metadata; this newer receipt establishes completed-command selection.
