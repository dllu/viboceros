# Point-cloud picking

[Architecture](architecture.md) · [Viewport implementation](viewport-implementation.md)

Top, Front, and Right click selection and Osnap use axis-aligned k-d trees instead of
scanning every cloud point. `PointCloud3` builds its XY index at construction;
XZ and YZ indexes are initialized on first valid use through `OnceLock` and
reused. Each additional index stores node metadata, not another copy of the
point coordinates. This trades first-use build time and retained index memory
for faster repeated queries. Callers that only query XY pay neither additional
index's construction or node-storage cost.

Cloning a cloud shares one immutable `Arc` data block instead of copying points
and index nodes. Lazy indexes initialized through any clone are available to all
clones of that data. Equality has a constant-time shared-storage fast path and
otherwise compares ordered points. Transformations create fresh data and fresh
indexes, leaving the original and its snapshots unchanged. The borrowed point
slice API and interchange representation are unchanged.

`point_cloud.rs` owns the public type, shared storage, and cache lifecycle.
`point_cloud/index.rs` owns k-d tree construction, deterministic ordering, and
bounded searches. `point_cloud/tests.rs` contains correctness, lifecycle,
concurrency, and opt-in timing checks; the public query API stays unchanged.

`PointCloudProjection` selects XY, XZ, or YZ for
`PointCloud3::nearest_projected_relative`. Queries keep the camera origin and
local cursor offset separate, evaluate distances in the selected model-space
plane, and preserve the earliest stored point on exact-distance ties. Radius
and offset validation happens before initializing an index. Existing XY APIs
delegate to the same implementation. Cloud equality depends on ordered points,
not cache state; transformed clouds rebuild their indexes from transformed data.

Each index node also stores its subtree's earliest source index (one additional
`usize` per node). Once a query finds a zero-distance hit, it visits eligible
children in earliest-source order and skips subtrees that cannot win the tie.
This avoids scanning every point when many different depths project onto the
same position. Nonzero-distance ties still use the ordinary spatial search.

The drafting API's `nearest_object_snap_axis_aligned` uses the same cached indexes
for clouds while retaining ordinary feature enumeration and priority rules for
other geometry. The camera's projection choice is shared by picking and Osnap.
Perspective selection and Osnap still scan projected cloud points, as do callers
of the generic arbitrary-projection snapping API. This is not an acceleration
of all geometry types, all snapping, or all viewport work.

## Validation and timing

Tests compare each projection with exhaustive searches at zero and large signed
translations, including zero-radius queries and ties. Other tests check lazy
initialization, reuse of an initialized index, cloud equality after cloning,
and translated pixel-capture boundaries in all three parallel views.
Lifecycle tests query clones made before and after cache initialization, retain
source-cloud query results after transformation, and verify transformed indexes
against transformed points. Four synchronized workers also exercise concurrent
first use through separate clones and reuse of the same published XZ/YZ node
buffers. A storage-sharing regression checks pointer reuse and queries a clone
after its source handle has been dropped.
Drafting tests compare axis-aligned snaps with the generic projected search over
mixed point/cloud/line scenes in all planes, including large signed translations,
locked targets, capture radii, and ties. Viewport tests check both points and
clouds at the Osnap pixel boundary in each parallel view.

Run the opt-in benchmark in release mode:

```sh
cargo test -p viboceros-geometry --release projected_index_query_benchmark -- --ignored --nocapture
```

It builds a deterministic 100,000-point cloud and compares 128 queries per
projection against an exhaustive scan, requiring identical results. Output
separates first-use index construction, warmed indexed queries, and scan time.
Timing is diagnostic, not a machine-dependent test threshold or a Rhino speed
comparison; it does not include rendering or document/UI overhead.

The same command also runs a coincident-projection fixture: 100,000 shuffled-depth
points, 128 warmed exact-hit queries, and an assertion that every result is the
earliest source point. A local before/after run measured 139–204 ms before subtree
source-index pruning and 0.037–0.040 ms afterward across XY/XZ/YZ. These are
diagnostic timings for this deliberately degenerate fixture, not a general
speedup claim. Regular tests check multiple coincident clusters, nonzero-distance
ties, radius boundaries, and the subtree source-index bounds themselves.

One local release-mode run produced the following totals for the fixture above:

| Projection | First-use build | 128 indexed queries | 128 scan queries |
| --- | ---: | ---: | ---: |
| XZ | 15.113 ms | 0.091 ms | 32.623 ms |
| YZ | 14.609 ms | 0.210 ms | 27.554 ms |
