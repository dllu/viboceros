# Standalone SelEdgeLoop workflows

[Command behavior](../commands/component-edge-loops.md) · [Oracle overview](../oracle.md)

`edge_loop_workflow_discovery` contains eight entry/accept/cancel controls.
`edge_loop_workflows` adds 36 controls covering intersecting loops, multiple
seeds on one loop, Ctrl removal, repetition, whole-object preselection and
ineligible preselected faces/vertices. Each family covers mesh and SubD grids.

The retained observations are immutable compressed native responses with
content-addressed producer snapshots. Each capture runs in private Xvfb and a
private Rhino settings scheme. It records before/after states, each seed pick,
EndCommand results, source CRC and a seeded Redo/Undo cycle. Escape can return
false from RunScript while EndCommand reports Success; final state is the
cancellation evidence. All 44 sources remain unchanged and redo survives.

The Rust protocol constructs its own 20-vertex, 12-quad grids. SubD creation uses
the same public mesh-conversion operation and corner tags; raw polygon creation
has a different component order. Tests compare all captured states, including
native component IDs, without supplying captured topology or output to Rust.
App tests separately replay picking, ambiguity, stale sources and ToNURBS handoff.
In-memory egui events verify bright seeds and dim loop previews in all three
display modes, without opening windows.

The existing Python API exposes these requests as `edge_loop_workflow` operations:

```python
import gzip, json
from pathlib import Path
from tools.rhino_oracle.client import OracleClient

base = Path("tools/rhino_oracle")
family = "edge_loop_workflows"
request = json.loads((base / "fixtures" / f"{family}.json").read_bytes())
expected = json.loads(gzip.decompress(
    (base / "observations" / f"{family}.json.gz").read_bytes()))
actual = OracleClient().run_viboceros(request)
keys = ("before", "trace", "after", "redo", "undo", "success", "source_unchanged")
for a, b in zip(actual["results"], expected["results"], strict=True):
    assert a["id"] == b["id"]
    assert all(a["value"][k] == b["value"][k] for k in keys), a["id"]
```

This compares selection lifecycle only. Earlier topology and ToNURBS curve-image
qualification remain separate; these planar grids do not establish nested-getter
behavior, ring selection or curved geometric error bounds.
