# Remembered command options

`ConvertToBeziers` remembers `DeleteInput`; `ConvertToSingleSpans` independently
remembers `Direction` and `DeleteInput`, including U/V toggles. Omitting an option
uses its last accepted choice. Updating only one option leaves the other alone.
[`ToNURBS`](commands/to-nurbs.md) separately remembers `DeleteInputObjects` and
`TrimTriangularFaces` after real conversions.
[`MeshToNURB`](commands/mesh-to-nurb.md) independently remembers
`TrimTriangularFaces` and `UseNgons`. Its native bootstrap choices are Yes/Yes;
the current mesh model has no n-gon regions, so `UseNgons` has no geometric effect.

Choices belong to typed command instances owned by `CommandRegistry`, not to
the document or its undo history. Aliases share an instance. Reusing a registry
across documents retains choices; separate registries are independent. The GUI
keeps its registry for the application session. The small `remembered` module
provides synchronized copy-in/copy-out storage without holding a lock during
geometry work or document mutation; the command API remains `Send + Sync`.

Preselected commands accept choices only after successful execution. Invalid options,
ineligible selections, and geometry/document failures leave previous choices
unchanged. An eligible single-span no-op accepts choices without changing
document history or clearing redo. A `ToNURBS` no-op leaves choices unchanged.
Undo/redo affects geometry, not these choices.

`MeshToNURB`'s [object prompt](object-selection.md) accepts options as they are
entered, independently of the eventual model transaction. Escape retains those
choices, including cancellation before picking anything. A failed conversion
also retains already accepted choices. Invalid option input changes neither the
pending options nor remembered values. Cancelling clears picks without editing
geometry, adding undo history, or discarding redo.

`ToNURBS` uses a separate confirmation phase and optional MeshOptions submenu.
Their edited values are staged, not remembered: only a successful real conversion
commits them. Escape from any phase discards those edits. No-op-only input skips
confirmation, including when selected during the command.

`ConvertToBeziers` asks a separate Yes/No deletion question after selection.
Answering converts immediately; Enter uses the displayed choice. Cancellation
at either stage accepts no choice, including an initial explicit native preset.
Preselection is retained on cancellation; command-first picks are cleared.

Native bootstrap choices are deletion No, Direction Both, and triangle trimming Yes. They are not
serialized across application restarts. Rhino's factory defaults and restart
persistence are not established by these tests. This is not a global preference
implementation for all commands; use explicit options in deterministic scripts.

The eight `conversion_sessions.json` probes run 39 steps in one command session,
creating fresh owned geometry for each step. Each command's first use explicitly
seeds every option, so probes cannot depend on Rhino's previous profile state.
Subsequent steps omit or partially change options, toggle U/V, interleave both
commands, accept no-ops, and undo real edits. Full conversion records are compared
after every step. Fixtures and macros are bounded and validated before execution;
an undo request on a no-op is rejected to avoid undoing unrelated history.
Six additional `nurbs_conversion_sessions.json` probes add 39 steps involving
`ToNURBS`, including interleaving all three commands. Its initial deletion choice
must be seeded by an actually convertible object; the first mesh must explicitly
seed triangle trimming. No-ops cannot make an unknown prior profile deterministic.
Three `mesh_nurbs_conversion_sessions.json` probes add 21 steps, seeding both mesh
options explicitly. The Rhino worker uses a separate owned mesh to enter the
option prompt, then measures the preselected command without normalizing its
selection. Triangle-trimming memory and its independence from `ToNURBS` are
verified; actual n-gon option effects remain unverified.
Three `mesh_nurbs_postselection_sessions.json` probes add 18 steps testing
cancelled choices before/after picking, mixed pre/postselection, undo, and
independence from `ToNURBS`. They exercise Rhino's actual selection prompt.
Three `nurbs_postselection_sessions.json` probes add 31 steps checking staged
ToNURBS choices, both cancellation paths, no-op inputs, undo, and interleaved
MeshToNURB memory. A cancelled ToNURBS command cannot seed a deterministic session.
Two `bezier_postselection_sessions.json` sequences add 24 steps checking both
cancellation stages, pre/postselection, omitted answers, and memory across undo.
A cancelled ConvertToBeziers command cannot seed a deterministic session either.
