# Native edge-analysis topology qualification

The `edge_analysis` oracle operation builds matching owned surface, B-rep and
mesh inputs in Viboceros and licensed Rhino 8. In Viboceros it executes the
actual `ShowEdges` session; in Rhino it reads public RhinoCommon topology APIs.
It does not invoke Rhino's Edge Analysis window or alter its model document.

The ten [recipes](../tools/rhino_oracle/fixtures/edge_analysis.json) cover a
standalone bilinear surface, a full-face trim, rectangular trim, rectangular
hole, reversed face, native box, welded and unwelded triangle pairs, a
three-face non-manifold side and a quad. The [raw observations](../tools/rhino_oracle/observations/edge_analysis.json)
retain every edge index, All/Naked/NonManifold flag, curve domain and seventeen
uniform parameter samples. Array order and endpoint direction are compared
directly; no fields, seams or domains are excluded or normalized.

All ten recipes and 2,268 numeric fields pass at 1e-9 absolute plus 1e-12
relative epsilon. The maximum absolute difference is
1.1102230246251565e-16. [Comparison](edge-analysis-native-comparison.json) and
[provenance](edge-analysis-native-provenance.json) preserve the exact source,
fixture, observation and validation-log hashes. The Rust integration replay
checks all fields on subsequent runs.

Mesh evidence uses [`TopologyEdges`](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.mesh/topologyedges),
`GetConnectedFaces`, `IsEdgeUnwelded` and `EdgeLine`; B-reps use
[`BrepEdge.Valence`](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brepedge/valence)
and the edge curve. Mesh All includes naked and unwelded edges, following the
[ShowEdges reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/showedges.htm).
Quads retain four sides and introduce no tessellation diagonal.

An initial suspected mesh ordering difference was disproved by source review
and the raw capture: OpenNURBS temporarily sorts locations, then orders welded
vertex IDs by each group's first original vertex index before sorting edge
pairs. Viboceros already follows that ordering for these recipes.

The capture ran in a private Xvfb display and private settings scheme. The
first attempt with native-first Wine security DLLs reached a license-refresh
dialog reporting a missing `AcquireCredentialsHandleW` entry point and ended
without observations. A process-local built-in security-DLL override succeeded:

```sh
WINEDLLOVERRIDES='sspicli,secur32,schannel=b' \
  tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/edge_analysis.json \
  --scheme VibocerosOracleEdgeAnalysis20261009 --timeout 240 \
  --output /tmp/viboceros-edge-analysis-native.json
cargo test -p viboceros-oracle --release --test edge_analysis
```

Wine configuration and license state were not changed. The launch error is a
process failure, separate from geometry comparison. This evidence qualifies
these topology records; native dialog controls, zoom framing, marked point
counts, broader curved and periodic surfaces, and extreme mesh coordinates
still require direct capture. It does not establish complete Rhino parity.
