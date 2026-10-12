# Native edge surfaces

Linux builds with `--features native-smlib` construct all two-, three- and
four-curve EdgeSrf outputs through usd-brep. The command retains its parsing,
selection, fresh output attributes, input groups and atomic Undo/Redo policy.
The portable backend remains available in other builds.

Both backends share the existing bounded boundary-order and reversal policy.
The native adapter imports each ordered curve once and works on private kernel
copies. Existing NLib operations normalize endpoint weights, synchronize paired
degrees and knots, construct ruled and Coons tensors, and elevate projective
controls when a finite Euclidean representation is needed. The kernel creates
the sheet face and spatial/parameter topology; the existing exporter validates
the owned Rust B-rep before document insertion. No custom tensor blending or
degree/knot matching runs in the native command or oracle construction path.

Coons parameter axes are aligned with the command using existing native NLib
copy/transpose operations. Unlike public `SwapUV`, this preserves full knot
data and supports mixed-sign controls. Extrusion also shares this operation,
removing its earlier chart expansion before swapping. Revolution still needs
its canonical working chart. All native calls remain under the adapter's kernel
lock; no native pointer enters document geometry.

The exporter certifies missing singular-side connectors using the complete
clamped natural-boundary control hull and the existing exact point-distance
predicate. This supports a positive collapsed boundary even when the interior
has mixed-sign weights. Other boundary shapes retain the general continuous
surface-image certificate. The point predicate was moved from the B-rep joining
code into the shared point module; the original arithmetic is reused.

Fresh Rhino 8.32 captures retain 33 API cases and eight actual EdgeSrf commands.
They cover lines, rational and mixed-degree boundaries, shifted domains,
reversals, permutations, gaps, triangular singularities and projective controls.
Native bridge regressions check all 33 surface and exported-sheet 13×13 grids
against those captures at absolute `2e-12`, relative `1e-14`, and retain the input
handles' complete definitions. Independent ruled tests also cover a ten-unit
bulge over a `1e-14` span at chart scales `2^-360`, `1` and `2^360`, including
the narrow-span midpoint. These are bounded qualifications, not continuous
Rhino parity for arbitrary inputs. Full raw comparison results are retained in
[smlib-edge-surfaces-comparison.json](smlib-edge-surfaces-comparison.json).
All eight command records and 32 of 33 API records match every raw field. The
two-multispan API case retains two distinct knots separated by rounding in the
native basis, while Rhino coalesces them, giving five versus four profile
controls. Its 13×13 geometry grid matches. The report also checks all 27 numeric
extrusion directions and the four short-span modeling records; the pre-existing
revolution parameter-domain difference remains visible.

Each input and compatible direction is limited to 512 controls. Open continuous
profiles and the modeling import's degree/weight limits still apply. Native
degree elevation attempts at most four steps when a zero-weight control needs
a finite representation. General charts near the explicit `1e-300` knot-match
threshold, extreme finite offsets, tiny-chart face topology and arbitrary
ambiguous boundary arrangements remain unqualified. The
[limitations ledger](usd-brep-limitations.md) distinguishes upstream API behavior
from exporter and command policies.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test edge_surfaces --test modeling_basis
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib edge_surface
```

The initial isolated live capture timed out. Both retained captures then
succeeded on an isolated display using the existing private oracle profile
`VibocerosOracleSmlibBridgeFinal20261010`. Harness timings do not establish
kernel speedups against Rhino.

Verification passed 2,129 geometry tests (eight ignored), 1,313 native command
tests (two ignored), 73 native bridge tests (one ignored) and 1,095 Python tests.
Portable EdgeSrf command and API checks, native and portable workspace clippy
with warnings denied, and workspace formatting also passed.
