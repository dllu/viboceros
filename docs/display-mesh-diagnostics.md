# Display mesh diagnostics

The optional `viboceros-io` example profiles the same portable B-rep display
meshing call used by the viewport. It uses the archive's stored tolerance and
defaults to eight samples per nonzero knot span. It emits JSON Lines to stdout
and progress to stderr, processing one object at a time so large meshes are not
retained together.

```sh
cargo run --release -p viboceros-io --example display_mesh_diagnostics -- \
  /path/to/model.3dm > /tmp/display-mesh.jsonl
```

Each object record contains its name, visibility, mesh vertex/triangle counts,
per-source-face triangle counts, and elapsed time. Support-face inventory includes
degree, control/knot/span counts, predicted untrimmed grid size, and trim sizes.
`--inventory-only` skips B-rep meshing. `--name SUBSTRING` filters names; repeat it
for alternatives. Hidden objects/layers are excluded unless `--include-hidden`
is supplied. The model header reports unsupported objects, which must be accounted
for before interpreting an assembly total.

## Retained ASTRA findings (12 October 2026)

The supplied native master contains 948 B-reps and 4,801 faces. The interactive
archive contains those same 948 B-reps plus eight mesh proxies, with the eight
expensive B-reps hidden. At resolution eight:

| Archive | Stored objects | Visible objects | Triangles | Stored mesh vertices | Meshing failures |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native master | 948 | 948 | 2,633,500 | 1,640,666 | 0 |
| Interactive | 956 | 948 | 1,194,760 | 763,388 | 0 |

The viewport expands every triangle to three 40-byte GPU vertices. These totals
therefore require 316,020,000 bytes (301.38 MiB) and 143,371,200 bytes (136.73 MiB).
The former buffer's old power-of-two capacity was exactly 536,870,912 bytes,
reproducing the reported 512 MiB allocation. The raw native data itself exceeds
256 MiB, so eliminating capacity rounding alone would not solve this assembly.
The renderer must respect the device limit when splitting allocations.

| Object family | Count | Triangles each | Largest support grid | Largest trim controls |
| --- | ---: | ---: | --- | ---: |
| Filleted instrument service cabinet | 6 | 239,890 | 3 U spans × 37 V spans | 124 |
| Elevation fork arm | 2 | 12,344 | 4 U spans × 1 V span | 1,069 |

Each cabinet has 26 faces, including 20 fillet supports with 37 V spans and
40 V control points. Twelve faces each generate exactly
`2 × (3 × 8) × (37 × 8) = 14,208` triangles. The fixed samples-per-span policy
creates 129,978 candidate support-grid points per cabinet. The six cabinets
account for 1,439,340 triangles; the two forks add 24,688. Together these eight
objects are 55.6% of the native model's triangles.

The forks have 356-span cubic edge/trim curves, rather than large support grids.
They each took about 15.5 seconds to mesh in the measured release run, while each
cabinet took about 2.63 seconds. The full native run took 47.42 seconds of summed
meshing time; the interactive run took 1.24 seconds. Timings are machine-specific
and do not isolate one particular internal meshing operation.

The source delivery's proxy generator uses native adaptive tessellation with
chord height `0.003`, curve angle `8°` and surface angle `10°` for Boolean/fillet
parts. Each cabinet proxy has 4,124 triangles. This provides a useful comparison
for future adaptive display meshing; the fixed-density algorithm has not been
changed by this diagnostic/import correction. Lowering its density globally
would change display quality without a geometric error bound.

## The six cabinets initially skipped on import

Before the correction, the portable reader returned 942 objects and six
unsupported objects. Independent OpenNURBS inspection identified the same error
on every cabinet:

```text
ON_Brep.m_T[51].m_iso = 4 and it should be 0
```

The archived trim had a stale derived South-isocurve flag. Every cabinet support
and trim NURBS form was individually valid. The existing private import copy
recomputed tolerances but retained these flags, so native B-rep validity rejected
the entire object before portable decoding. Recomputing `SetTrimIsoFlags()` on
that copy gives valid B-reps and restores all 948 objects, without fitting curves,
altering support geometry, changing topology, or modifying the source archive.

The tiny [derived-flag fixture](../crates/viboceros-io/tests/fixtures/derived-iso/README.md)
covers stale boundary and non-isoparametric flags. The optional independent
[`native_3dm_inventory.cpp`](../crates/viboceros-io/examples/native_3dm_inventory.cpp)
reports raw validity, the existing tolerance normalization, and validity after
derived-flag recomputation, along with native support/trim sizes. Compile it as
C++17 linked to the same public OpenNURBS archives described in the
[independent pushup audit](../crates/viboceros-io/tests/fixtures/pushup/README.md),
then run it with an archive path and optional name substring.

Input SHA-256:

```text
71235a8b4ebd678c0ba912e576b985af25f4a098eb02867d562ba7191729da32 ASTRA-07-Native-Master.3dm
fa99521511990df67ea419c448d4cc6fb64878fb01f8c83087e0de7397bb2f9f ASTRA-07-Interactive.3dm
```

The real-model archives are optional user-supplied fixtures and are not vendored.
