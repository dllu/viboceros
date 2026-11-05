# Edge-analysis lifetime and geometry events

`ShowEdges` remembers requested source membership while objects are deleted.
Missing sources disappear from the display and edge cache; Undo restores their
analysis automatically. Explicit Remove and ShowEdgesOff forget membership.
Color, mode, appearance and pending Redo survive display refreshes.

Geometry replacement creates a new runtime object version. Undo/Redo restore
the appropriate version and runtime number. A separate transient geometry-event
sequence tracks admission and replacement, including history replay. Diagnostics
use this sequence for source order. UUIDs, persistent table positions, geometry
storage and model equality remain stable through restoration. Runtime metadata
is excluded from model debug snapshots and file interchange.

Refresh scans live objects, orders their geometry events and prunes cache entries
against live IDs. It avoids the previous per-source linear document lookup.
A 512-object test verifies deleted-cache eviction, surviving-cache reuse,
restored membership and preservation of Redo.

## Qualification

Native Rhino 8 captures run only through private Xvfb. Panel Zoom captures use
physical clicks on the owned button and public command-begin events to request
fixed keyboard options. The host targets the owned process window and records
receipts for every option; no in-process keystroke timers drive the active Get.

| Family | Records | Coverage |
| --- | ---: | --- |
| `edge_analysis_lifetime` | 13 | Scripted deletion/Undo/Redo, single/all sources, smooth/flat and explicit Remove controls |
| `edge_analysis_lifetime_runtime` | 29 | Scripted runtime-version traces; repeats deletion cases and adds Move/Flip |
| `edge_analysis_lifetime_ui` | 29 | The same version and geometry workflows with physical panel input |
| `edge_analysis_panel_selection` | 14 | Mode controls and physical Zoom with selected analyzed or unrelated sources |
| `edge_analysis_modes` | 11 | All mode transitions, repeated activation, fresh All default and remembered mode |

Command, app and release Python replays compare complete source definitions,
selection, presence, runtime-order ranks, ordered duplicate endpoints and every
history stage within `1e-12`. App tests resolve analysis between Delete and Undo,
exercise live Zoom options, and verify original geometry storage after restore.
Mode controls preserve selection. Zoom retains persistent preselection and
releases temporary getter picks on completion. The panel uses analyzed sources
even when unrelated objects are selected.

Six failed timer/input attempts retain their error traces and frozen producers
as diagnostic records, without successful geometry results. A separate parallel
startup timeout is also retained; its serialized retry succeeded. Native captures
are run serially in the shared Wine prefix. The earlier mixed scripted-control
records also remain immutable. New physical-input traces show
why manual/persistent selection and temporary Add/Remove picks have different
cleanup. The document models both states, restores persistent selection through
Undo, and preserves temporary state on rollback. [Mixed-source evidence](mixed-edge-analysis.md)
documents the physical UI references. Precise native camera framing, highlight
pixels, document replacement and broader topology still need qualification.

```sh
cargo test -p viboceros-document --lib
cargo test -p viboceros-command --lib edge_analysis
cargo test -p viboceros-oracle --test edge_analysis_lifetime --test edge_analysis_mixed
cargo test -p viboceros --bin viboceros edge_analysis
python3 -m unittest tools.rhino_oracle.test_edge_analysis_lifetime tools.rhino_oracle.test_edge_zoom_input
python3 -m tools.rhino_oracle.edge_analysis_lifetime_replay
```

Public API context: [object selection and persistent selection](https://developer.rhino3d.com/api/rhinocommon/type/rhino.docobjects.tables.objecttable/?member=select-system.guid).
