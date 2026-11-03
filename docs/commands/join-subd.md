# Join SubDs

[Join and JoinCopy](join.md) accept SubD objects in both preselection and
command-first workflows. The selection prompt exposes `SubDJoinedEdges`:

```text
Join SubDJoinedEdges=Smooth
JoinCopy SubDJoinedEdges=Crease
```

The default is Smooth. Each command remembers its option independently of model
history. Joined boundaries become smooth or hard creases; unrelated corners,
interior creases and soft sharpness survive. Limit geometry changes where the
joined control net acquires different subdivision rules. Mixed-family joining
and the `SubDSymmetry` option remain unsupported.

Contacts use boundary control vertices and the document absolute tolerance.
Admitted vertices retain the earlier assembly's coordinates. Vertex-only
contacts can connect cages; complete duplicate faces remain separate.
Inputs stay immutable until all results have been validated. No display mesh
conversion participates in joining.

Preselection scans document order, reconsidering inputs reached through later
connections. Command-first picks extend the initial seed once per pick; an
unrelated earlier pick is not revisited. A single source fails unchanged.
Disconnected or duplicate preselected sources succeed unchanged without new
objects or model history. An unsuccessful command-first batch retains its seed
selected at command completion.

Outputs inherit the seed's attributes and groups. Preselected outputs stay
selected; command-first outputs are unselected. JoinCopy retains participating
originals, selected at command completion, and preserves their exact geometry.
Postselected picks are released when model history is replayed. Edits form one
Undo/Redo step, and rejected inputs do not consume Redo.

Twenty-six frozen public SDK cases compare control points, raw face/edge order,
vertex/edge tags and every control vertex's evaluated limit point, with a
`1e-10` position epsilon. Thirty-six command cases cover both commands,
selection modes, crease options, attributes and groups. A separate clean-macro
suite records command completion, script return, Undo and Redo: Rhino's script
runner releases command-first selections after the EndCommand event. Replays
compare both phases explicitly; app tests use SelNone for that separate driver
cleanup. Display mesh ordering and pixels are not qualified.

In the recorded three-way boundary competition, Rhino retains one unchanged
source while also including its face in a joined nonmanifold result. The batch
implementation reproduces that behavior; command-first picking skips the
competing source. Arbitrary overlapping cages, symmetry associations, packing
layouts and parallel source edges need further qualification or implementation.

Contact discovery uses a spatial sweep. A shared budget bounds discovery and
admission to 16 million record visits and one million contacts, including retries
on dense duplicate cages. Inputs are limited to 100,000 controls/faces, 400,000
face corners and 64 MiB per assembled archive. Independent checks include
2,000 disconnected inputs, source immutability, appended soft creases and native
3DM round trips of joined outputs with asymmetric soft seam weights.

```sh
cargo test -p viboceros-geometry --lib subd_join
cargo test -p viboceros-command --lib join::
cargo test -p viboceros-oracle --test subd_join
cargo test -p viboceros --bin viboceros subd_join_native
python3 -m unittest tools.rhino_oracle.test_subd_join
```

References: [Join](https://docs.mcneel.com/rhino/8/help/en-us/commands/join.htm),
[public SubD SDK](https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.subd/).
