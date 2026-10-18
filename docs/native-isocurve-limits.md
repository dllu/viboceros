# Native one-sided isocurve limits

`N_SrfExtractIsoCrv` now extracts the actual surface limit at unclamped
endpoints and full-order knot groups. The repair compiles a generated copy of
the pinned native source; the upstream checkout remains unchanged.

The previous endpoint shortcut copied the outermost control row regardless of
the basis. That is correct at a normally clamped end, but fails for unclamped
padding and for full-order groups with inactive outer controls. The old interior
path also rejected multiplicity greater than the degree.

The repaired producer locates the exact knot group and span for an in-domain
query. Full-order groups select their isolated row: the right limit at the start
or an interior break, and the left limit at the active end. Other endpoints use
the existing homogeneous de Boor reduction on the appropriate nonzero span.
The varying degree, full knot vector and control count remain unchanged. No
fitting, knot snapping, new basis evaluator or tolerance change is involved.
The prior out-of-domain extrapolation route is retained without a new claim
about its accuracy.

The checked Rust-facing adapter now sends non-full-order endpoints directly to
this producer. Its recursive endpoint refinement and obsolete classification
check are removed. It retains reversible binary chart scaling, control/weight
conditioning, input/resource validation, complete output-basis validation and
the exact payload copy for isolated rows and spherical poles.

The direct probe covers 288 cases across both directions, positive and negative
common weight signs, chart widths `2^-40`, `1` and `2^40`, and origin `1e12`.
Cases include linear, quadratic and cubic unclamped endpoints; partial endpoint
multiplicity; ordinary interior stations; full-order interior breaks; and
inactive rows at both ends. Independent closed-form coefficients specify every
expected homogeneous control. Dyadic cases require exact equality; other cases
use `2e-14 * max(1, abs(expected))` per homogeneous coordinate. Every case checks
both preallocated and stack-allocated native extraction, complete varying knots,
source purity and returned curve lifetime. The varying basis includes a
single-ULP last span. A separate out-of-domain object request must leave both
outputs null without changing its source.

The original-object control passes 128 of the 288 limit cases; the repair
passes all 288 through both native allocation paths. All four probe modes also
pass under AddressSanitizer with leak detection disabled for the process-global
context. The release bridge suite passes 411 tests and the command suite passes
1,600, with zero failures and four existing ignored tests. Clippy and formatting
pass. All 112 saved raw join outcomes remain unchanged.

Fresh Rhino 8 capture succeeds for all fifteen existing isocurve fixture cases.
Their complete raw protocol outputs match the current native extractor at
absolute and relative tolerances `1e-10`. The
[saved SDK observation](../tools/rhino_oracle/observations/surface_isocurve_geometry.json)
is retained unchanged. Those fifteen captures do not cover every combination in
the independent 288-case producer matrix.

The opt-in target `viboceros_smlib_isocurve_limits_baseline` links the original
`NL_SrfTool.cpp` object ahead of the repaired archive while retaining the existing
curve-ownership and UV repairs. This isolates the limit-selection change.
The [validation record](native-isocurve-limits-validation.json) retains its raw
control, candidate and sanitizer results, completed suite logs and source hashes.

```sh
cargo test -p viboceros-smlib --features native \
  --test isocurve_pairs --test isocurves --test face_creases
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/surface_isocurve_geometry.json \
  --observations tools/rhino_oracle/observations/surface_isocurve_geometry.json \
  --native-smlib --timeout 1800
```

General optional trimmed-domain pairing, arbitrary numerical ranges, native
remote-chart ray queries and complete Rhino command parity remain unqualified.
No kernel speedup or leak-free result is claimed.
