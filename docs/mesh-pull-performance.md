# Mesh Pull performance work

Local release measurements compare the implementation at `151fd7d5` with the
current geometry changes. Both binaries use `native-smlib`, one warmup and one
timed iteration per workload, repeated in three fresh oracle processes. The
table reports median command/probe time in milliseconds at absolute tolerance
`1e-4`. Fixture construction is outside the measured closure; document cloning,
Pull execution and output arc-length sampling are inside it.

| Workload | Before (ms) | After (ms) |
|---|---:|---:|
| Rational circle, two faces | 933.27 | 785.55 |
| Line, 8×8 grid (128 triangles) | 22.54 | 23.04 |
| Line, 24×24 grid (1,152 triangles) | Work limit | 557.40 |
| Rational line, weight ratio 1,000,000 | 510.05 | 0.150 |

The weighted line traverses its exact degree-one span chord, retaining reversals
and constant spans without tessellating parameter speed. General curve
approximation reuses exact de Casteljau subdivisions when the native midpoint
is exactly halfway; other midpoints still restrict at their actual parameter.
Long chord queries can use smaller conservative source intervals to reduce
competing triangles. The existing work limits and geometric bounds are retained.

The smaller grid did not improve. These are three-run local measurements under
ordinary host load, not statistical or Rhino speed qualification. Circle
pulling and dense meshes still have substantial cost. Closest-branch stability,
native command parity and the existing branch/precision/resource limitations
remain as documented for [mesh Pull](commands/pull-mesh.md).

Retained reports contain every raw outcome, timing, request digest and executable
digest. Geometry checks verify analytic lengths and loci for successful results;
the prior dense-grid failure is retained. Source provenance and validation are
in [the qualification record](mesh-pull-performance.json).

Build each revision with the same release feature set, retaining the baseline
executable before rebuilding, then measure either binary:

```sh
cargo build --release -p viboceros-oracle --features native-smlib
python3 -m tools.rhino_oracle.mesh_pull_performance \
  --binary target/release/viboceros-oracle --repeats 3 --output /tmp/mesh-pull.json
python3 -m unittest tools.rhino_oracle.test_mesh_pull_performance
```
