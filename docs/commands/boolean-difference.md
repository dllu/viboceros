# BooleanDifference

[Command reference](README.md) · [Kernel and limits](../convex-booleans.md)

```text
BooleanDifference
BooleanDifference DeleteInput=No
BooleanDifference DeleteCutters=No
BooleanDifference FirstSet=1,2 SecondSet=3,4
```

Select surfaces or polysurfaces to subtract from, then press Enter. Select the
cutters and press Enter again. Preselection accepts the target set immediately.
At least one target and one cutter are required. Points and unrelated objects
are filtered out. First-set objects cannot also be picked as cutters. Explicit
ID sets execute directly for scripts.

`DeleteInput` and `DeleteCutters` default to `Yes`. Both can be changed during
either selection phase. Accepted changes are remembered by the command registry
across documents, cancellation, failure, and Undo/Redo. Invalid option batches do
not change preferences. `DeleteCutters` is hidden while `DeleteInput=No`; its
remembered value returns when input deletion is enabled again.

- `DeleteInput=No` retains all input geometry, IDs, metadata, groups, and
  selection, then adds the results.
- `DeleteInput=Yes` deletes the accepted targets after a successful operation.
  `DeleteCutters=Yes` also deletes all accepted cutters, including disjoint ones.
  `No` retains the cutters and their selection.

## Document behavior

Each target is cut separately by its interacting cutters. Overlapping targets
remain separate results. If another target cuts successfully, disjoint targets
are also copied to new IDs. Fully removed targets produce no replacement; an
operation can succeed with zero output objects. Strictly interior cutters are
ignored, including when another cutter intersects the target boundary. Disjoint,
equal, strictly nested, and point-only touching pairs produce no replacement on
their own. Face and edge contacts can succeed with unchanged target volume.

Preselection follows document table order; command-first picking follows pick
order. Each result inherits attributes, layer, color, and groups from its target.
A single remainder retains target geometry user text. Multiple disconnected
pieces from one target lose geometry user text, matching the captured splits.
Source group memberships do not select unrelated peers. Disconnected output order can differ from native
results; `SelLast` can consequently refer to a different piece. Coplanar ownership
remains unverified beyond the saved recipes.

Outputs are selected when targets were preselected; command-first outputs are
unselected. Retained inputs stay selected at command completion. Comparisons use
the native EndCommand state because the script runner subsequently releases
transient picks. First-phase cancellation clears its picks. Second-phase
cancellation releases cutter picks and retains accepted targets. Cancellation
changes no geometry or model history. Failed computations end the prompt and
retain input geometry and selection.

All model changes form one undo step. History replay releases transient picks,
retains preselected target selection on Undo, and restores selected outputs on
Redo. Retained cutter picks from command-first execution are released on history
replay.

## Current scope and native evidence

Inputs must be individual closed, convex, manifold polyhedral shells satisfying
the exact affine polygon certificate. Multiple original cutters can produce
nonconvex remainders and separate material components. Curved, open, nonconvex,
multi-shell, input-hole, and singular input representations remain unsupported.
Resource exhaustion and unresolved topology return errors without partial model
changes. The geometry API also supports mathematical enclosed cavities; the
command follows Rhino's captured boundary-interaction policies.

The [40 native recipes](../../tools/rhino_oracle/fixtures/boolean_difference_command.json)
and [saved observations](../../tools/rhino_oracle/observations/boolean_difference_command.json)
were captured on private Xvfb with Rhino 8.32.26160.13001. Command tests replay
44 computation outcomes, including [eight supplemental split and coplanar cases](../../tools/rhino_oracle/observations/boolean_difference_order_command.json),
comparing retention, EndCommand selection, attributes,
groups, geometry user text, face/edge counts, mass properties, and finite
bidirectional polygon-boundary witnesses. Disconnected pieces are matched within
each target by location; raw ordering differences are retained in the captures
and reported by the replay. One coplanar recipe explicitly records 13 faces in
both results and 31 kernel edges versus 33 native edges: different cap and side
overlap ownership creates two additional native collinear junctions. All other
count, metadata, mass-property, and boundary assertions remain unchanged. These
internal seams are not native-equivalent. The tests replay preselected and
command-first Undo/Redo and successful option memory. Application tests cover
both phases, filtering, preselection, explicit sets, option visibility and memory,
cancellation, and history. See [capture provenance](../boolean-difference-provenance.json).

Absolute comparison epsilons are `1e-10` for volume and centroid, `1e-9` for area,
and `1e-7` for boundary witnesses. Different supporting surface parameterizations
and internal coplanar partitions are permitted. Continuous boundary error,
arbitrary geometry, native component ordering, internal seams, and native kernel
speed remain unproven. Native event statuses remain in the capture; the command
API returns messages and errors.

```sh
cargo test --release -p viboceros-geometry brep::boolean
cargo test --release -p viboceros-command boolean_difference -- --nocapture
cargo test --release -p viboceros boolean_difference
python3 -m unittest tools.rhino_oracle.test_boolean_difference
```

Capture again in a fresh private scheme:

```sh
tools/rhino_oracle/run_headless.sh exec python3 - <<'PYCODE'
import json
import uuid
from pathlib import Path
from tools.rhino_oracle.client import OracleClient
from tools.rhino_oracle.boolean_difference_probe import request
scheme = 'VibocerosOracleDifference_' + uuid.uuid4().hex[:16]
result = OracleClient(settings_scheme=scheme).run_rhino(request(), timeout=600)
Path('/tmp/boolean-difference-capture.json').write_text(json.dumps(result, indent=2)+'\n')
PYCODE
```
