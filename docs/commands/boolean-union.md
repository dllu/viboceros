# BooleanUnion

[Command reference](README.md) · [Kernel and limits](../convex-booleans.md)

```text
BooleanUnion
BooleanUnion DeleteInput=No
BooleanUnion MergeCoplanarFaces=No
```

Select at least two supported polysurfaces before invoking the command, or start
it and pick objects. Enter completes picking; Esc cancels. Points and other
unrelated objects are filtered out. Options can be entered while picking.

Both options default to `Yes` and are remembered by the command registry across
documents. Accepted changes survive cancellation, failure, and Undo/Redo. Invalid
option batches do not change preferences.

- `DeleteInput=Yes` removes participating originals and creates new result IDs.
  `No` retains their geometry, IDs, metadata, groups, and selection.
- `MergeCoplanarFaces=Yes` removes internal boundaries between adjacent coplanar
  faces. `No` combines fragments of each original face while retaining boundaries
  between faces from different inputs. Eligible collinear edge subdivisions are
  removed in either mode.

## Document behavior

Preselection uses document table order; command-first picking uses pick order.
Each output inherits attributes, layer, color, and group memberships from its
first boundary contributor. Geometry user text comes from its last boundary
contributor, including coplanar patches owned by another face. Strict interior
inputs are consumed when a surrounding union succeeds, without taking ownership
of geometry user text. Group memberships do not select unrelated peers.

Preselected results are selected. Command-first results are unselected; retained
originals and disjoint singleton bodies stay selected at command completion.
The native script runner releases transient picks after its EndCommand event;
application comparisons use that event's state. Command-first picks are also
released during Undo/Redo. Geometry, attributes, metadata, and group membership
changes form one undo step.

Without a boundary interaction, the command reports no changes and retains the
originals. Disjoint, equal, strictly nested, and point-only touching pairs do not
produce a replacement. A disjoint singleton remains unchanged if other selected
bodies successfully union. Cancellation changes no geometry or model history.

## Current scope and native evidence

Inputs must be individual closed, convex, manifold polyhedral shells satisfying
an exact affine polygon certificate. Multiple original inputs can produce a
nonconvex result, cavities, or separate material bodies. Curved, open, nonconvex,
multi-shell, and singular input representations remain unsupported. Work and
output limits return errors without partial model changes.

The [32 native command recipes](../../tools/rhino_oracle/fixtures/boolean_union_command.json)
and [saved observations](../../tools/rhino_oracle/observations/boolean_union_command.json)
were captured on private Xvfb with Rhino 8.32.26160.13001. Command tests replay
27 main outcomes: 22 closed successes, four unchanged pairs, and one insufficient
selection. Tests compare source retention, selection at EndCommand, attributes,
groups, geometry user text, face/edge counts, mass properties, and finite
bidirectional polygon-boundary witnesses. They also replay captured Undo/Redo.
Application tests cover filtered picking, minimum selection, immediate
preselection, cancellation, remembered options, failure completion, and history.
See [capture provenance](../boolean-union-provenance.json).

Volume and centroid comparisons use absolute epsilon `1e-10`, area `1e-9`, and
boundary witnesses `1e-7`. These finite witnesses permit different supporting
surface parameterizations and internal coplanar face partitions. For example,
the captured equal-sized overlapping boxes assign coplanar overlap to later
inputs; the kernel uses the earliest input. Internal wireframe seams and raw
face/edge ordering are consequently not native-equivalent. Continuous boundary
error, arbitrary input geometry, and native kernel performance remain unproven.

The capture retains two successful cases outside this scope: an open surface
mixed with closed solids, and boxes meeting only along an edge that yield a
nonmanifold open result. These require additional kernel support. Rhino's
`Success`, `Failure`, `Nothing`, and `Cancel` event codes are retained in the
capture; the command API currently returns messages and errors instead of those
four native status values.

```sh
cargo test --release -p viboceros-geometry brep::boolean
cargo test --release -p viboceros-command boolean_union
cargo test --release -p viboceros boolean_union
python3 -m unittest tools.rhino_oracle.test_boolean_union
```

To capture again without using the shared desktop, choose a fresh private scheme:

```sh
tools/rhino_oracle/run_headless.sh exec python3 - <<'PYCODE'
import json
import uuid
from pathlib import Path
from tools.rhino_oracle.client import OracleClient
from tools.rhino_oracle.boolean_union_probe import request
scheme = 'VibocerosOracleUnion_' + uuid.uuid4().hex[:16]
result = OracleClient(settings_scheme=scheme).run_rhino(request(), timeout=480)
Path('/tmp/boolean-union-capture.json').write_text(json.dumps(result, indent=2)+'\n')
PYCODE
```
