# Native surface splitting and trimming

Linux `native-smlib` builds route isocurve Split, rectangular visible-face
preparation, separate ExtendSrf output and round Pipe caps through the shared
usd-brep directional extractor introduced for surface seams. The native adapter
exports `trim_surface_direction`, `trim_surface` and `split_surface`.

The geometry policy validates increasing intervals and strictly interior split
parameters, bypasses exact whole-domain no-ops, and verifies returned degrees,
intervals and the complete unselected basis. Rectangular extraction edits U before
V and validates both bounds before backend entry. Existing control-row builders
now propagate caller error types; the portable splitter computes each row once
instead of repeating the split for its second output.

Higher-order axes batch checked native curve extraction under one kernel lock.
Two-control axes reuse the curve adapter: polynomial lines keep the existing
exact affine path, while rational lines still use usd-brep. This preserves exact
plane crossings without another interpolation solver. Native handles stay
outside document state and sources are immutable. Command preparation propagates
native failures through its existing typed error conversion.

Unchanged fully clamped natural endpoints retain their complete original control
payloads. Previously, projecting independently conditioned columns rounded an
exact spherical pole into a tiny ring, preventing a tapered Global Pipe cap from
closing. The endpoint fix preserves exact singular topology without changing
tolerances. The pole-payload regression and Local/Global Pipe volume checks
exercise this behavior.

Isocurve Split outputs restore 3D edge domains from the original surface axes
using exact knot translations, including reversed north/west sides. Internal
construction still evaluates in its lossless local frame. The existing general
rectangular-face constructor keeps its local-edge policy; the new surface-domain
variant is used by isocurve Split. Degree, weights, controls, UV trims and topology
are unchanged by that chart translation.

The [native regressions](../crates/viboceros-smlib/tests/surface_segments.rs)
compare full definitions and 9×9 grids against the portable reference and original
source. They cover rational partial rectangles, periodic curves, near-knot bounds,
binary charts `2^-360`, `1`, `2^360`, common weight gauges `1e-300`, `1`, `1e300`,
`-1e300`, independent row gauges, full-order interior breaks, inactive endpoint
controls, exact plane crossings, pole payloads, source purity and invalid requests.
Shared-policy tests check callback order, whole-domain bypass, returned layouts
and typed backend errors. Edge-chart tests include ordinary and `1e12` origins.

Fresh Rhino 8.32 captures retain [11 direct SDK trims](../tools/rhino_oracle/observations/surface_trim_geometry.json)
and [eight actual isocurve Split commands](../tools/rhino_oracle/observations/surface_split_isocurve_command.json)
from isolated Xvfb under `VibocerosOracleSmlibBridgeFinal20261010`.
Ten complete trim definitions match at absolute `1e-10` and relative `1e-12`;
all eight complete command records match at absolute `1e-8` and relative `1e-12`.
The [comparison](smlib-surface-segments-comparison.json) retains each outcome and
input/observation hashes. One deliberate whole-domain no-op retains four unused
full outer knot slots that Rhino reconstructs differently; every stored knot,
control and the independent 9×9 grid matches. That raw discrepancy is retained.
All 346 additional retained editing, seam, decomposition and intersection records
still match at their established tolerances. Capture timings include the harness.

Validation passes 3,585 distinct geometry/command/bridge tests (11 ignored),
including the complete native suites, the additional SDK definition regression
and the edge-chart regression. Six affected command tests were rerun after edge
domain restoration. Both workspace lint configurations pass with warnings denied;
formatting passes. The Python oracle suite passes all 1,097 tests. Its historical
signed-length provenance now points to the immutable original SDK archive while
the refreshed observation remains linked to its own extraction qualification.

The [limitations ledger](usd-brep-limitations.md) explains why the adapter uses
checked directional extraction instead of the snapping tensor convenience API.
Its native degree/resource/gauge restrictions still apply. These finite cases
do not certify arbitrary chart or weight ratios, continuous surface parity, or
kernel speedups.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test surface_segments
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/surface_trim_geometry.json \
  --scheme VibocerosOracleSmlibBridgeFinal20261010 --timeout 240 \
  --output tools/rhino_oracle/observations/surface_trim_geometry.json
```
