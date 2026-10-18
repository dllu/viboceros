# Native surface joining

Linux builds with `--features native-smlib` route the B-rep geometry stage of
Join and JoinCopy through usd-brep's existing `SmStitch` executive. Rust retains
command selection policy, source ownership, metadata, groups and Undo/Redo.
The default Rust boundary matcher is compiled into the command adapter only
when the native feature is disabled.

Stitching operates on a native copy and preserves every source face. Face
indices are carried through the kernel and checked as a permutation before
export. Exported NURBS and topology pass the shared Rust B-rep validation;
edge-connected components carry their original source indices. No native
pointer enters a document. Partial circular overlaps are split and joined by
the upstream kernel, extending the Rust matcher's straight partial-overlap
coverage without another matching algorithm.

The shared importer also accepts [unclamped edges, UV trims and surfaces](smlib-unclamped-import.md).
Native canonical construction clamps each active domain before stitching;
JoinCopy keeps the original objects' unclamped definitions. This can change
output coefficients and exterior knots while preserving the active surface.

Native orientation now uses an [exact U/V permutation](native-uv-transpose.md)
for surface bases and trim coordinates. It no longer rebuilds a surface through
canonical knot cleanup or reflects trims through a rounded diagonal frame.
Translated active domains and narrow interior spans survive the operation;
this does not change native edge-matching admission or resolve every chart scale.

Native UV creation can update tolerances, so export snapshots edges and vertices
after the trims. Tiny out-of-domain UV controls use native knot snapping only
within a roundoff allowance, followed by the existing continuous surface-image
certificate. Larger excursions and unproved corrections remain errors.

The executive uses precise edge comparison, manifold edge pairing and vertex
splitting. Small-edge squeezing and sliver removal are disabled to retain
source geometry. Individual glue failures leave boundaries open. Solid-region
nesting during stitching is disabled for sheet assemblies; the native
orientation operation consistently unifies normals using topology. For every
newly closed edge-connected component, the command reuses its existing exact
spatial orientation witness to correct a provably inward result, including when
unrelated open sheets remain. This needs no representable volume. Unknown
witnesses remain unqualified. A wholly closed assembly containing an unknown
new component retries once from original sources using the previous native
ray-orientation path, retaining its results or errors. This check is not a
nesting or solid-validity certificate. The [translated closed-box qualification](native-closed-join.md)
checks source geometry, outward command orientation and complete history.

Extreme spatial extents are translated and scaled before import, then restored
on export. Their edge/surface domains are mapped to `[0,1]` together with the
UV trims; scaling coordinates alone was insufficient. Parameterization can
therefore change. Native orientation can also swap a face's UV axes. Join now
restores the input chart when the exported NURBS is its exact U/V transpose,
using checked source-face indices. It compensates the face sense, trim
traversal and UV metadata without changing native spatial geometry or oriented
normals. Other parameterizations retain the native chart. See the
[source-chart regression and remaining differences](join-source-uv.md).

The requested command radius remains below 1.8 model tolerances for preselection
and 2.1 for command-first picking. SMLib additionally uses inherited entity
tolerance sums and retains candidate vertices rather than averaging them.
These produce documented differences from the prior gap behavior; stored input
tolerances are not reduced to force a cutoff. Native Join can also stitch naked
boundaries within an input, whereas the Rust matcher searches across inputs.
See the [limitations ledger](usd-brep-limitations.md) for verified findings.

Qualification currently includes copied exploded-box closure and analytic volume,
duplicate-sheet assembly, partial circular-overlap connectivity and analytic
area, normalized extreme domains, and the command's existing selection,
metadata and history regressions. Existing Rhino comparison scores for Join
describe the default backend; they do not qualify this native route. Broader
raw Rhino replay, ambiguity, corner cleanup, performance and arbitrary imported
topology remain ongoing work. Input sheet components are limited to 128 by the
shared importer. Command-first participation probes use copied two-source
kernel operations in addition to the growing batch assembly. They now compare
only the newest pick against original accepted sources, stopping at its first
confirmed contact. At most `n` source assemblies replace `1 + n(n-1)/2` for an
attempt with `n > 2` sources. A source assembly whose closed orientation remains
unknown may repeat native stitching once for the ray-orientation fallback. A batch with no surviving cross-source join needs
no participation probes. See the [operation counts and unchanged replay](join-pick-probes.md).

The [October 11 Rhino replay](smlib-join-comparison.json) retains all 66 original
surface-command cases and 26 discrepancy cases. Every native operation now
executes without an audit error; 14 of 92 cases match every raw field at
`1e-10` absolute / `1e-12` relative epsilon. The other 78 retain explicit
tolerance, parameterization, orientation, ordering or geometry differences.
Execution success does not establish boundary equivalence. The report retains
native responses, fixture/observation hashes, source hashes and the probe hash;
audit timings are placeholders and supply no performance evidence.
The [October 17 chart comparison](join-source-uv-comparison.json) separately
retains the current before/after responses for the 26 discrepancy cases;
source-chart restoration does not make this strict archive pass.

Release validation on October 11, 2026 passed 1,304 command tests and 44 native
bridge tests, with two command tests and one bridge test ignored. The default
backend's 21 Join regressions also passed. Native clippy with warnings denied
and workspace formatting checks passed.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test stitching
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib join::
```
