# BooleanIntersection

[Command reference](README.md) · [Kernel and limits](../convex-booleans.md)

```text
BooleanIntersection
BooleanIntersection DeleteInput=No
BooleanIntersection FirstSet=1,2 SecondSet=3,4
```

Start the command and select the first set of surfaces or polysurfaces. Enter
accepts that set. Select a second set and press Enter to intersect the unions of
the two sets, or press Enter with an empty second set to compute the common
intersection of every object in the first set. Common intersection requires at
least two objects. Preselection accepts the first set immediately. Points and
unrelated objects are filtered out; first-set objects cannot also be picked into
the second set. Explicit ID sets are available for scripts.

`DeleteInput` defaults to `Yes`. It can be changed in either selection phase.
Successful changes are remembered by the command registry across documents and
Undo/Redo. Cancelled changes are discarded. `No` retains the input geometry,
IDs, metadata, groups, and selection. `Yes` deletes all accepted inputs when a
result is produced, including disjoint members of a two-set operation.

## Document behavior

Preselection uses document table order; command-first picking uses pick order.
Ordinary common-set processing reduces inputs in order. Untouched enclosing
inputs are skipped; a smaller input can replace an earlier intersected region
and take its metadata. Boundary interaction determines the surviving
contributors: the first supplies attributes, layer, color, and groups, and the
last supplies geometry user text for one output. Multiple outputs clear that
text. See [ordered common intersection](../common-intersection.md) for the exact
rules and 80 native witnesses. Ordinary two-set results inherit attributes
from the first maximal first-set contributor and geometry user text from the
last maximal first-set contributor. A maximal pair region is not contained in
another pair region within that output component. These policies reproduce the captured recipes;
arbitrary overlapping contributor configurations remain unverified. Group
memberships do not select unrelated peers.

Compound two-set inputs use the measured [oriented shell
pipeline](../compound-intersections.md). Each set is combined before shell-pair
processing. Enclosed or disjoint copies can retain their own intermediate
metadata, and separate output objects can overlap.
Compound common intersection exports participating connected boundaries of its
material intersection. Complete unchanged boundaries take attributes from their
matching input, and inactive enclosing inputs supply no result metadata. Measured
inward pairs and non-solid edge contacts are documented in
[oriented pairs and common intersection](../compound-pairs.md).

Common-set outputs are unselected. Two-set outputs are selected when the first
set was preselected; command-first outputs are unselected. Retained originals
stay selected at command completion. Comparisons use the native EndCommand
snapshot because Rhino's script runner subsequently releases transient picks.
Geometry, attributes, metadata, and groups form one undo step. Transient command
picks are released during history replay.

Esc or `Cancel` during first-set picking clears its picks. Cancellation during
second-set picking retains the accepted first set and releases second-set picks.
Cancellation changes no geometry or model history. A failed computation ends
the prompt and retains the inputs. Disjoint and strictly nested pairs produce no
replacement; equal and face-touching pairs also fail to produce a volumetric
replacement. Boundary-contained solids with an intersecting boundary can succeed.

## Current scope and native evidence

Closed embedded polyhedral inputs can have concave faces, holes, multiple shells,
cavities, and straight edges from earlier Boolean results. The convex certificate
selects the faster construction when available; otherwise the general exact
polyhedral certificate applies. Curved and open inputs, intersecting input
shells, unsupported plane roundoff, resource exhaustion, and uncertified singular topology
return errors before model edits. See [chained command scope and compound shell
policies](../polyhedral-boolean-commands.md) for the additional native evidence
and explicitly unresolved intersections.

The [32 native recipes](../../tools/rhino_oracle/fixtures/boolean_intersection_command.json)
and [saved observations](../../tools/rhino_oracle/observations/boolean_intersection_command.json)
were captured on private Xvfb with Rhino 8.32.26160.13001. Command tests replay
27 computation outcomes, comparing retention, EndCommand selection, attributes,
groups, geometry user text, face/edge counts, mass properties, and finite
bidirectional polygon-boundary witnesses. Captured Undo/Redo and remembered
successful options are replayed. Application tests cover both selection phases,
preselection, pick order, filtering, cancellation, remembered options, and
history. See [capture provenance](../boolean-intersection-provenance.json).

Absolute comparison epsilons are `1e-10` for volume and centroid, `1e-9` for area,
and `1e-7` for boundary witnesses. Finite witnesses permit different supporting
surface parameterizations and internal coplanar partitions. Internal seams and
raw face/edge ordering are not guaranteed native-equivalent. Continuous boundary
error, general curved geometry, all preference behavior after computation
failure, and native kernel performance remain unproven. Native event status codes
are retained in the capture; the command API returns messages and errors.

```sh
cargo test --release -p viboceros-geometry brep::boolean
cargo test --release -p viboceros-command boolean_intersection
cargo test --release -p viboceros boolean_intersection
python3 -m unittest tools.rhino_oracle.test_boolean_intersection
```

Capture again in a fresh private scheme:

```sh
tools/rhino_oracle/run_headless.sh exec python3 - <<'PYCODE'
import json
import uuid
from pathlib import Path
from tools.rhino_oracle.client import OracleClient
from tools.rhino_oracle.boolean_intersection_probe import request
scheme = 'VibocerosOracleIntersection_' + uuid.uuid4().hex[:16]
result = OracleClient(settings_scheme=scheme).run_rhino(request(), timeout=600)
Path('/tmp/boolean-intersection-capture.json').write_text(json.dumps(result, indent=2)+'\n')
PYCODE
```
