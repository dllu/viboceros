# BooleanSplit trimmed sheets and compound inputs

[BooleanSplit](commands/boolean-split.md) · [Finite sheets](boolean-split-plane-cutters.md) · [Open targets](boolean-split-open-targets.md)

BooleanSplit now distinguishes outer sheet coverage from enclosed trim holes.
A finite sheet whose outer loop covers the section may cut a solid even when
an inner loop removes part of the cap. The resulting bodies can be open. A
sheet ending inside the section still produces no split. Cap faces retain the
actual trim holes and original supporting surface.

The mathematical solid-partition API retains its complete-cap requirement.
`split_polyhedral_brep_by_trimmed_sheets` separately queries material components
with outer coverage, then exports only physical patches. Logical component and
lineage queries remain exact; inner missing patches do not silently become caps.
The open-target report includes each branch's connected-component count so the
command can preserve or clear geometry user text independently of attributes.

For a compound closed target, the command drops untouched shells from a
successful split's outputs. It retains the original full object when
DeleteInput=No and restores it on Undo. The mathematical partition functions
retain their material semantics; command participation is resolved separately.
A compound open target can repeat an untouched coplanar component in each
shared/unshared branch. Those disconnected branch outputs lose geometry user
text in the saved native cases. Attributes and groups always come from the target.

Original trim holes differ from holes created by an earlier stage. Coplanar
cutters that overlap an originally trimmed sheet can create a filled primary
boundary plus disconnected remainders, including a small patch inside the
original hole. Exactly filling an original hole without positive-area overlap
produces no command split. A perpendicular sheet crossing an original trim hole
also fails in the capture. Existing staged-hole behavior remains independently
covered by the mixed-stage fixtures.

Fourteen owned public recipes ran on private Xvfb under
`VibocerosOracleBooleanSplitTopologyPilot20261007` with Rhino 8.32.26160.13001.
Twelve succeed and create 33 pieces; two failures remain in the
[raw records](../tools/rhino_oracle/observations/boolean_split_topology.json).
They retain original/output boundaries, face/edge counts, area, applicable solid
mass properties, metadata, groups, idle selection, events and Undo/Redo. Replay
checks every outcome, boundary witnesses in both directions at `1e-7`, scalars
at `1e-9`, solid volume/centroid at `1e-10`, source retention, geometry-user-text
lineage and history. Both target and cutter compounds, original holes, cap holes,
preselection and DeleteInput=No are represented. Kernel checks explicitly keep
the strict solid API distinct from open boundary output; app tests verify the
full original compound returns on Undo. See [provenance](boolean-split-topology-provenance.json).

General curved/nonplanar geometry, higher-degree supports, arbitrary overlapping
shells, broad compound/trim combinations, near-contact tolerance and relative
performance still need implementation or evidence. Native insertion order remains
a known difference. The recorded outcomes do not certify general Rhino parity.

```sh
cargo test --release -p viboceros-command boolean_split_topology
cargo test --release -p viboceros-geometry trimmed_sheet_boundary
cargo test --release --bin viboceros boolean_split_compound_target
python3 -m unittest tools.rhino_oracle.test_boolean_split_topology
```
