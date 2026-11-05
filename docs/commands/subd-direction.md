# SubD direction

[`Flip`](flip.md) reverses open SubDs while preserving control points, tags,
component IDs and object metadata. Closed SubDs are skipped. [`Dir`](direction.md)
inspects both, previews open-SubD reversal, accepts with Enter and discards with
Escape. Two reversals restore the original storage and preserve pending Redo.
SubDs do not offer UV-axis edits.

Preselection survives accepted edits and Undo/Redo. Canceling inspection clears
even a single SubD preselection. Command-first picks remain deselected through
Undo/Redo, including skipped closed SubDs. Mixed SubD/line workflows follow the
same cleanup without expanding their groups.

The inspector draws at most 256 exact limit face-center normals per SubD.
Hover interpolates cached display normals at the picked mesh point, including
flat control-net appearance, and applies the provisional orientation. Display
mesh interpolation approximates smooth normals between exact native samples;
native arrow positions, density and pixels have not been matched. Persistent
`ShowDir` membership for SubDs remains outside this implementation.

## Evidence

Two immutable Rhino 8 families retain 82 named-command workflows:
[discovery](../../tools/rhino_oracle/fixtures/subd_direction_discovery.json) (30)
and [controls](../../tools/rhino_oracle/fixtures/subd_direction_controls.json) (52).
They cover bent quads, open/closed cubes, creases, a pentagon and valence-seven
fans; smooth/flat display, both selection routes, mixed groups, repeated edits,
cancellation and seeded pending Redo. Captures run inside private Xvfb sessions.

Rust and application replays compare complete states before, after, Undo and
Redo: coordinates within `1e-12`, exact face cycles, tags, component IDs and
selection. Independent sources use the same public mesh-conversion route to
give IDs corresponding meanings. Undirected edge endpoints are canonicalized;
raw observations retain their original order. Discovery macros include an
extra Enter after `Flip`; the controls capture bare `Flip` too. Comparisons use
settled snapshots, since EndCommand can precede getter selection cleanup.

Rhino reports `Nothing` for SubD-only `Dir`, even after accepting a geometry
change, and also on Escape. Mixed SubD/line sessions report `Success` on Enter
and `Cancel` on Escape. Raw events and success flags remain unchanged in the
captures. Portable replay reports semantic `completed` separately and checks
geometry, selection and history directly.

```sh
cargo test -p viboceros-oracle --test subd_direction
cargo test -p viboceros --bin viboceros subd_direction
cargo test -p viboceros --bin viboceros viewport::direction
cargo test -p viboceros-command --lib direction::session
python3 -m unittest tools.rhino_oracle.test_subd_direction
python3 -m tools.rhino_oracle.subd_direction_replay
```

Raw gzip responses and hashed producer snapshots live in
`tools/rhino_oracle/observations/`. The release Python replay needs no Rhino or
desktop window. This is bounded behavioral evidence, not full SubD command parity.
