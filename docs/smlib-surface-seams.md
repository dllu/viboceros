# Native surface seams

Linux `native-smlib` builds route SrfSeam geometry through usd-brep. The shared
geometry policy checks closure and periodic topology for the complete stored
homogeneous control net, then edits U before V when Both is selected. Constant
pole rows do not need to form standalone closed curves. Unselected parameters
are ignored; invalid selected parameters and closure failures are typed geometry
errors. Native errors propagate through the command's existing conversion.

`NurbsSurface::try_change_closed_seam_with_operations` accepts whole-tensor
refinement and split callbacks. Simple periodic seams use the existing native
tensor InsertKnot operation once per direction. Other interior seams use two
directional strip extractions. The C ABI batches every selected control row or
column under one kernel lock, reusing the checked native curve extractor and
its per-line chart/gauge conditioning. It verifies consistent output bases and
copies the complete unselected degree, count and knot vector unchanged.
Native-owned intermediate snapshots use RAII cleanup, including failures.

Periodic control rotation, knot translation and clamped homogeneous append use
the same geometry helpers as curve seams. Start seams remain exact no-ops and
end seams translate one period without entering the backend. The portable
surface path uses the same policy; its split computes each control curve once.
No additional refinement, fitting or evaluation solver is introduced.
Document identity, attributes, groups, selection and history stay in the command
layer, and native handles never enter document state.

The [repaired native rectangle extractor](native-surface-patches.md) now retains
exact requested boundaries and handles unclamped/full-order endpoint groups.
It clamps both directions, so the seam adapter still uses checked directional
curve extraction to preserve the complete unselected basis. The historical
[bilinear probe](../tools/usd_brep/patch_extraction_probe.cpp) and
[retained result](smlib-surface-seams-probe.json) record requested U start `1e-9`
with an incorrectly snapped X boundary `0`; the current generated repair removes
that snapping and remapping. See the [limitations ledger](usd-brep-limitations.md)
for source evidence and remaining split workarounds.

The [native regressions](../crates/viboceros-smlib/tests/surface_seams.rs) compare
the eight SDK fixture definitions, full knots, degrees, counts, periodic flags,
controls, weights and 9×9 grids against the portable reference and original
wrapped source chart. They exercise U, V and Both, rational multiple knots,
binary chart scales `2^-360`, `1`, `2^360`, and common weight scales `1`, `1e100`,
`-1e100`. Additional pole-row cases retain an unclamped unselected basis, test
clamped and periodic seam paths near a knot at `3 + 1e-9`, and transpose both
axes. Shared-policy tests verify one whole-tensor callback per operation,
unchanged unselected layouts, exact endpoint bypasses and invalid-input rejection.
Polynomial and rational Both cases also compare U-then-V against transposed
V-then-U preparation.

Fresh [Rhino 8.32 SDK captures](../tools/rhino_oracle/observations/surface_change_seam.json)
contain all eight fixtures, collected on isolated Xvfb under private scheme
`VibocerosOracleSmlibBridgeFinal20261010`. Every complete native definition matches
at absolute `1e-10` and relative `1e-12`; the
[raw comparison](smlib-surface-seams-comparison.json) retains each result and
fixture/observation SHA-256 hashes. The additional 338 retained curve-editing,
decomposition, splitting and intersection cases also match at their established
tolerances, including the numeric-length `1e-6` ceiling. Timing records include
the harness and do not establish kernel speedups.

The original seam qualification covered 3,575 distinct tests: 3,448 geometry/command cases, 125 native
bridge cases and two additional debug policy regressions (11 ignored across the
full suites). Workspace lint passes in both feature configurations with warnings
denied, and formatting passes. Undo/redo tests compare generated snapshots
exactly while allowing numeric native projection differences in controls.

The inherited homogeneous closure policy still rejects the tested tiny common
weight gauge before backend entry. General weight/chart ratios, continuous
surface parity and performance remain unqualified. Definition geometry uses
tight numeric comparisons; full knots, metadata and undo/redo snapshots retain
exact comparisons.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test surface_seams
cmake --build /tmp/viboceros-usd-brep-build \
  --target viboceros_usd_brep_patch_extraction_probe -j 10
/tmp/viboceros-usd-brep-build/viboceros_usd_brep_patch_extraction_probe
```
