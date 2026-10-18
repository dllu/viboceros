# Native knot refinement

Linux `native-smlib` builds now delegate InsertKnot's curve and tensor-surface
refinement to usd-brep's existing NLib operations. Command parsing, target
multiplicity, U/V/Both direction, symmetry, object conversion, metadata and
atomic history follow the existing command flow. Native builds exclude the
portable geometric insertion routine from this command and its oracle probes.

The adapter accepts the complete source definition, including unclamped outer
knots and periodic padding. It constructs private NLib data under the shared
kernel lock, calls existing refinement or endpoint-split operations, and copies
the complete native result into the common owned NURBS payload. Canonical
modeling import is not involved, so both ends are not clamped before the edit.
No new control-blending, basis-insertion or endpoint-extraction solver is added.
Binary common-weight scaling protects homogeneous arithmetic without changing
the result's stored gauge. Inputs remain unchanged.

The shared geometry policy restores periodic control/knot padding and the two
artificial outer entries that OpenNURBS does not store. Both backends now use
the same curve completion helper. Surface repair uses the existing whole-net
periodicity predicate; a single row cannot decide whether a rational tensor
with unequal seam weights is periodic. This policy changes representation
around the active interval without another geometric refinement.

The [native split repair](native-exact-splits.md) fixes full-order and unclamped
endpoint allocation/copy bounds. The adapter now splits directly at either end,
without native reversal wrappers. Clamped/no-op endpoints return copied
definitions, and one-sided clamping retains the opposite end's original data.
An endpoint group with full multiplicity can still have inactive controls and
exterior padding. For that case, the adapter copies the retained Euclidean
controls and full knots directly, preserving exact payloads without a
homogenization/projection round trip. The shared outer-knot policy retains
an exterior entry when duplicating its neighbor would exceed full multiplicity.
These arbitrary full vectors need not be representable by OpenNURBS' omitted
outer entries; independent qualification covers them without a Rhino parity claim.

Fresh Rhino 8.32 responses retain eight curve and five surface API operations.
All thirteen complete raw definitions match at absolute `1e-10`, relative
`1e-12`: degree, domains, control coordinates, weights, full knots and periodic
state are checked. They include rational, periodic, multiple and unclamped-end
cases. The [comparison](smlib-knot-refinement-comparison.json) retains every
outcome and input hashes without comparison-time normalization.

Independent bridge tests retain source definitions and check 65 curve points
and 17×17 surface grids. Additional tests cover a ten-unit bulge over a `1e-14`
span at parameter scales `2^-360`, `1`, `2^360`; common gauges `1e-300`, `1`,
`1e300`, `-1e300`; and unclamped clamping at both ends in both tensor axes.
These are bounded fixture checks, not a continuous parity certificate for all
possible input definitions. The command regressions also cover Both/symmetry,
analytic conversion, selection, attributes, source identity, Undo and atomic
invalid-input rejection.
The inactive-control regressions additionally check retained controls exactly,
valid complete basis reconstruction, repeated clamping, both tensor axes,
degree elevation/reduction and deformable fitting. The portable insertion
path uses its existing endpoint extraction before treating a full-order request
as a multiplicity no-op.

The pinned NLib degree limit remains 17 and the payload limit is one million
controls. Refinement that creates a zero-weight homogeneous control with no
finite Euclidean representation fails explicitly. Weight ranges whose binary
gauge would underflow are rejected. Arbitrary scale extremes and all full-order
interior discontinuity cases remain further qualification work.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test knot_refinement
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib insert_knot
```

Both live captures ran on isolated displays with private profile
`VibocerosOracleSmlibBridgeFinal20261010`. Comparison timings include the probe
harness and do not establish kernel speedups.

Latest verification passed 3,543 native-feature tests across geometry, command
and bridge crates (11 ignored), all 43 retained raw Rhino cases across native
knot/degree/nonperiodic editing, portable endpoint regressions, and workspace
clippy in both feature configurations with warnings denied. Workspace formatting
also passed.
Earlier integration qualification passed all 1,095 Python oracle tests and
portable command and oracle knot checks.
