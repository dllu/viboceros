# MeshTrim continuations

[MeshTrim](commands/mesh-trim.md) retains its original cutting geometry for the
whole command. Trimming or deleting a cutter does not remove its ability to cut
other targets. Surviving pieces retain their original target identity for
self-exclusion: an original cutter must not cut its own descendants. Local Undo
restores a previous piece without losing either relationship.

`MeshTrimCutters` holds shared immutable geometry snapshots and tracks accepted
descendant IDs. Capturing does not create document objects, copy mesh buffers or
edit history. Each accepted pick commits through the existing history group;
failed picks roll back independently. Descendant IDs are registered only after
commit, and their relationships remain available through local Undo. The getter
owns both the captured inputs and its history token.

```rust
let mut cutters = MeshTrimCutters::capture(&doc, &cutting_ids)?;
let mut history = doc.begin_history_group("MeshTrim")?;
cutters.trim(&mut doc, target_id, point, &[], context, &mut history)?;
```

The [qualification](mesh-trim-continuation-qualification.json) retains ten native
workflows and replays nine backend and ten getter workflows. Seven backend and
eight getter workflows match surfaces, metadata, vertex-channel samples,
selection, source storage and Undo/Redo. These include a trimmed mesh cutter
whose removed portion still divides a different target: Rhino retains area 1.2,
where losing that cutter would retain only 0.8. Two perspective-view cases with
curve cutters produce different surfaces; their cause remains unqualified.
The earlier [MeshTrim qualification](mesh-trim-qualification.json) remains a
separate measurement set, including its temporary-plane difference.
