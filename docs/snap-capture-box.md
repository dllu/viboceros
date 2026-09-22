# Square object-snap admission

[Interface](interface.md) · [Point probes](point-snaps.md) ·
[Mesh competition](mesh-snap-order.md) · [Provenance](snap-capture-box-provenance.json)

Object snaps now use an inclusive square aperture. `capture_radius` means its
half-width in projected units: a target is admitted when both cursor-relative
coordinates have absolute value at most that width. Euclidean distance still
computes curve targets and ranks admitted features. No mesh-ranking heuristic
was added.

The change covers discrete features, indexed parallel-view point clouds,
projected cloud scans, Near, mesh Mid, and curve-hover Center/Mid. Conservative
curve/mesh bounds also use the square. Straight hover calculations retain local
screen offsets, avoiding world-coordinate reconstruction that would lose cursor
precision at large origins. The circular point-cloud picking API is unchanged;
the new square query reuses the same indexes and source-order tie handling.

## Retained Rhino evidence

All 200 new GetPoint observations came from newly owned private-Xvfb sessions of
Rhino 8.32.26160.13001, using public APIs only. Inputs, complete observations,
actual camera/click calibration, unchanged sources and restored settings are
retained. Source-only generators do not consume observed target coordinates.

The [128 aperture inputs](../tools/rhino_oracle/fixtures/snap_capture_box.json)
and [observations](../tools/rhino_oracle/observations/snap_capture_box.json) contain:

- 64 corner checks across Top/Perspective, line Near/End and mesh Near/Mid.
- 48 slanted-wire controls: four rotations, both endpoint orientations,
  line/mesh representations, and half-widths 8, 12 and 16.
- 16 line Mid-only checks distinguishing curve hover from direct capture.

There are 70 captures and 58 non-admissions. Of the non-hover captures, 39 lie
outside the inscribed circle but inside the square. The ten successful Mid-only
curve hovers place their targets more than three aperture half-widths away;
the nearby locus point, not the distant Mid target, determines admission.
Six slanted line controls at half-width 12 cross the square yet are rejected:
their Euclidean closest points lie outside it. Clipping the target to the box or
minimizing Chebyshev distance would incorrectly admit them. Two rotated line
controls capture a nearest point about 12.101 pixels away inside that square.

Native replay agrees on **all 128 admission decisions and snap kinds/owners**,
and, after the [endpoint follow-up](mesh-snap-endpoints.md), on 123 full 3D targets
including non-admissions, at componentwise absolute `1e-9` with zero relative
epsilon. The initial checkpoint had 115 matches and thirteen differences:

- Five corner cases choose a different wire/target from minimum screen distance.
- Eight short-wire cases at half-width 16 return a wire endpoint rather than the
  predicted interior point **on the same reported topology edge**. These are new
  counterexamples to the per-wire calculation, not merely wire-selection issues.
  All four rotations and both endpoint orientations retain them.

Those eight short-wire cases now match: two endpoints inside the aperture select
the screen-nearest endpoint. The five competing-wire differences remain unresolved.

The [72 representation inputs](../tools/rhino_oracle/fixtures/mesh_snap_sources.json)
and [observations](../tools/rhino_oracle/observations/mesh_snap_sources.json) compare
the same competing Near wires as separate lines, separate meshes, and a combined
mesh, with reversed source order in Top and Perspective. All 48 separate-source
cases match native point, kind and owner. Seven of the 24 combined-mesh cases
differ; overall replay is 65/72. This localizes those competition discrepancies
to selection inside one mesh rather than the between-object ranking rule.

At the initial checkpoint, 60 of 68 captured mesh Near targets agreed with the
independent per-wire formula on the observed component; all 68 now agree after
the endpoint correction. All original observations are preserved. The earlier 101-case
and 84-case corpora remain unchanged, as do their 98/101 and 35/84 native matches.
Tests that preserve known differences are not Rhino compatibility passes.

## Reproduction and limits

```sh
python3 -m tools.rhino_oracle.references.snap_capture_box --all
python3 -m tools.rhino_oracle.references.mesh_snap_sources
python3 -m tools.rhino_oracle.point_snap_replay tools/rhino_oracle/fixtures/snap_capture_box.json tools/rhino_oracle/observations/snap_capture_box.json
python3 -m tools.rhino_oracle.point_snap_replay tools/rhino_oracle/fixtures/mesh_snap_sources.json tools/rhino_oracle/observations/mesh_snap_sources.json
python3 -m unittest tools.rhino_oracle.test_snap_capture_box
cargo test --release -p viboceros-oracle projected_object_snap
```

Both replay commands intentionally return parity-failure status 1 while these
differences remain. For fresh owned Rhino captures, generate the aperture stages
separately (no flag for 64 discovery cases; `--held-out` for 64 controls), then use
`tools/rhino_oracle/run_headless.sh rhino REQUEST --timeout 600`.

New native regressions cover square boundaries, closer outside-square cloud
points that must not hide an admitted corner, brute-force agreement in all three
parallel projections at large signed origins, lazy index reuse, distance
overflow, curve/polygon hover, and slanted-wire rejection. Existing exhaustive
mesh-BVH and rational projected-line checks remain enabled. Curved proximity is
still a bounded search, not a certified general global minimizer. These probes
do not establish full mesh endpoint/selection parity, occlusion, arbitrary-scene
compatibility, model-history behavior, or a Rhino performance advantage.

Validation checkpoint at `2949124`: 3,152 release workspace tests pass (29 ignored), along
with all 328 Python tests, warning-denied Clippy/rustdoc, formatting and whitespace
checks. All four real replay CLI runs retain their documented parity-failure
status and counts; no epsilon was widened to obtain these results.
