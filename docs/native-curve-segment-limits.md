# Native curve segment limits

`N_CrvExtractCrvSeg` now uses exact in-domain knot groups and their actual
multiplicities. It retains the correct one-sided restriction at unclamped ends,
inactive endpoint groups and full-order breaks. A generated-source repair
preserves the existing knot-removal repair and leaves upstream unchanged.

The old active-end shortcut replaced the right span with the final knot index
and assumed multiplicity `degree+1`. It skipped the required end restriction on
an unclamped curve and could retain inactive suffix controls. Its general span
helper also skipped short endpoint spans by an absolute tolerance. The repair
changes classification, retaining the existing homogeneous restriction code and
allocation contracts. The prior span lookup outside the active domain remains
unqualified.

The checked Rust-facing trim adapter now passes the complete source directly to
that producer. Its private reverse/split/reverse end clamp and inactive-suffix
removal are deleted. Binary chart scaling, coordinate/weight conditioning,
resource validation and complete output-basis checks remain. Full-order bounds
copy their isolated original endpoint payloads exactly, including interior
groups and inactive padding. Group lookup uses binary searches and reuses their
ranges for the expected interior knots. Whole-domain no-ops, isolated whole
blocks and exact affine line crossings retain their existing policies.

The direct probe has sixteen families and 512 configurations. Degrees `1`, `2`,
`3`, `5` and `17` cover Bernstein restrictions, a thin interval, uniform and
irregular unclamped quadratics, an unclamped cubic, inactive endpoints and
full-order cubic breaks. Chart scales are `2^-40`, `1`, `2^40`, plus origin
`1e12`; common weight gauges are `1`, `-1`, `2^-300`, `2^300`, with polynomial
and rational controls. Expected controls come from independent polar-form
polynomial products, uniform-span identities and isolated blocks.

Allocated output, preallocated output with a null allocation stack, and in-place
output are checked separately. Every degree, count and full knot is exact;
homogeneous errors divided by the common gauge must be at most
`5e-13 * max(1, abs(expected/gauge))`. Separate source and output stacks allow
the allocated and preallocated outputs to be reread after source destruction.
Out-of-place source data remain unchanged; in-place input is consumed.

The public adapter also runs all 512 configurations. It preserves 128 complete
no-op definitions and 96 isolated endpoint payloads exactly. Partial results
match the independent controls at absolute `1e-10` and weight ratio `1e-12`;
17 stations per configuration compare the resulting loci at absolute `1e-10`.
The original-object control passes 312 cases in each native allocation path.

The opt-in target `viboceros_smlib_curve_segment_baseline` overrides only the
preceding generated `NL_CrvTool.cpp`, retaining the earlier knot-removal repair.
The [validation record](native-curve-segment-limits-validation.json) retains raw
controls, instrumented runs, SDK definitions and comparisons, completed checks
and source hashes. AddressSanitizer runs target invalid memory access and
allocation lifetime; leak detection is disabled and no leak-free claim is made.

Fresh Rhino SDK capture covers nine continuous fixtures, including degrees up to
17. All eight partial-trim definitions match in all three native paths at
absolute `1e-10`, relative `1e-12`. The whole-domain cubic retains an unclamped
basis in Rhino while the native primitive clamps it; that raw mismatch remains.
Rhino also evaluated both unchanged definitions at 65 identical parameter
stations. Points and first derivatives agree within `3.6e-15` in those sampled
comparisons, which are separate from the strict definition reports.

Both nine-case checked-adapter replays match eight complete definitions, keeping
the expected whole-domain outer-knot padding mismatch. All 112 saved join
outcomes remain unchanged. Release validation passes 414 bridge tests and 1,600
command tests, with zero failures and four existing ignored tests. Workspace
lint with warnings denied and formatting pass.

The underlying `N_CrvSplit` full-order indexing defect and unclamped native
endpoint evaluation remain separate limitations. No arbitrary degree/range,
continuous Rhino parity or kernel speedup is claimed.

```sh
cargo test -p viboceros-smlib --features native \
  --test curve_segment_limits --test curve_segments --test directed_segments
```
