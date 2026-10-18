# Native Join source UV charts

Native Join and JoinCopy use usd-brep's copied stitching operation. Native
surface orientation sometimes transposes a face's U/V axes, including when
importing a reversed open sheet. Export previously exposed that transpose as
different surface coefficients and trim coordinates.

The native kernel's [exact transpose repair](native-uv-transpose.md) also keeps
remote active intervals and narrow spans intact during orientation. That repair
prevents canonical reconstruction from changing the surface before export.

The adapter now restores the corresponding input chart when the exported
NURBS is exactly its transpose. The kernel's checked face-origin permutation
identifies the source. Degree, knot vectors, control net and rational weights
must match exactly after transposition. Clamping, domain normalization or any
other coefficient change leaves the native chart in place. This does not fit
geometry or match boundaries again.

The shared geometry operation transposes the surface and trim coordinates,
reverses each loop's traversal and edge-use directions, exchanges U/V trim
tolerances and iso labels, and toggles the face sense. Spatial vertices and
edges retain their storage and values. Oriented surface normals, boundaries,
holes and signed volume are preserved. A batch of face changes commits only
after every index and replacement succeeds; invalid or duplicate indices
leave the B-rep unchanged. Join's existing outward-orientation correction for
newly closed components still runs separately.

Geometry tests cover both face senses, holes, cylinder seams, sphere singular
trims, rational surfaces, anisotropic trim tolerances, inverse chart changes
and atomic errors. Native stitching tests cover reversed exploded boxes,
including an inward native result whose signed volume must remain negative,
a reversed sheet with a hole, and curved rational sheets. Original Rust and
native operands remain unchanged. These tests check geometry and orientation
independently of Rhino's raw table layout.

The [comparison report](join-source-uv-comparison.json) retains all 26 original
discrepancy cases against the unchanged saved Rhino observations, with
[baseline](join-source-uv-native-before.json) and
[candidate](join-source-uv-native-after.json) response values. Both replays
complete. Raw differences decrease from 7,301 to 7,289; none of these cases
matches every field. In all four triple-boundary cases, raw underlying surface
coefficients now match Rhino. Their zero-volume joined pair retains the
previously documented complementary face senses. Stored tolerances, table
order, gap behavior and other differences remain. These counts compare raw
serialization, not physical boundary distances or performance.

The unchanged native regression
`original_discrepancy_archive_tracks_resolved_partial_and_gap_joins` advances
past its prior face-sense assertion, then fails on a stored edge tolerance
(`1e-9` versus Rhino's `0`). Its comparisons and saved observations have not
been relaxed. The [validation record](join-source-uv-validation.json) identifies
the baseline, source hashes, passing checks and this known failure.

Reproduce the candidate and raw comparison from the repository root:

```sh
cargo run -p viboceros-oracle --features native-smlib -- \
  tools/rhino_oracle/fixtures/join_surface_differences.json /tmp/join-source-uv.json
python3 tools/rhino_oracle/join_source_uv_comparison.py \
  docs/join-source-uv-native-before.json /tmp/join-source-uv.json \
  tools/rhino_oracle/observations/join_surface_differences.json /tmp/join-source-uv-comparison.json
cargo test -p viboceros-geometry face_uv
cargo test -p viboceros-smlib --features native --test stitching
cargo test -p viboceros-command --features native-smlib join::
```
