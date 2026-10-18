# Native interpolation at supplied parameters

`N_FitCrvInterpGivenParams` now assembles the actual basis row at every supplied
parameter. Its former identity endpoint rows fitted incorrect unclamped limits;
its degree-one shortcut copied samples into controls even when those samples
were between the knots. The linear fast path remains for distinct knot stations.

The producer orders unsorted constraints together in private scratch storage,
preserving the caller's points and parameters. A degree-wide band includes
unclamped endpoint support. The existing native banded LU and point/homogeneous
back solver remain; no new basis evaluator, linear solver or modeling tolerance
is introduced. Dimensions, finite monotone knots, active-domain stations and
finite point channels are checked. Duplicate parameters and rank-deficient
support are rejected. A successful result must have finite control channels.
Native error returns can leave partially allocated output; callers must check
status before publishing it.

The original linear-precision witness now recovers controls
`[-0.5,0.4,1.4,2.5]` and endpoint limits `[0,2]` on the quadratic basis
`[-2,-1,0,0.8,2,3,4]`. The old result and its incorrect limits remain in the
[historical probe](smlib-curve-rebuild-probe.json).

The regression matrix covers 2,040 reconstructions: 17 families, four charts,
three parameter orders and allocated/preallocated outputs. Degrees are
`0`, `1`, `2`, `3`, `5`, `11` and `17`; charts use `2^-360`, `1`, `2^360` and
origin `1e12`. Polynomial Euclidean input, polynomial `NL_CPOINT` input and
rational homogeneous input exercise both native representations. Rational
common gauges are `1`, `2^-180` and `-2^180`; a coordinate channel at `1e-18`
must retain its relative detail. Shuffled constraints, interior-only samples,
unclamped uniform/irregular bases and a full-order cubic break are included.

Independent Bernstein and piecewise polynomial identities generate sample data
from known controls. The fitted degree, count and complete knots must match
exactly. Control errors normalized by each channel's maximum magnitude are
bounded by `2e-8`; the observed maximum is below `9.2e-12`. After destroying
the input arrays, 33 stations per result check native homogeneous and Euclidean
evaluation: `2e-11` per-channel relative homogeneous error and absolute `1e-10`
Euclidean error. The observed maximum point difference is below `7.2e-15`.
Raw controls retain `NL_NOW`; only homogeneous comparisons map that polynomial
sentinel to unit weight. No representation change is hidden in raw results.

Thirteen additional cases check duplicate constraints, deficient support,
inactive basis padding, malformed dimensions/knots, nonfinite inputs and an
unrepresentable solution. The opt-in original-object control executes only the
2,040 well-formed reconstruction cases: it passes 360. Malformed inputs are not
fed to the original producer because it lacks the corresponding guards.

Fresh Rhino capture evaluates the retained expected and fitted definitions at
90 paired parameter stations on ten continuous curves. Maximum point difference
is below `5.4e-15`; first-derivative differences are retained as diagnostics.
This checks sampled loci through the public SDK, rather than comparing Rhino's
interpolation producer to this internal fitting routine.

The [validation record](native-point-interpolation-validation.json) retains raw
controls, witness output, sanitizer runs, SDK requests/responses and completed
checks with source hashes. The upstream checkout remains unchanged; generated
copies compile the repair.

Release suites pass 416 bridge tests and 1,600 command tests, with zero failures
and four existing ignored tests. All 2,053 direct probe cases pass under
AddressSanitizer. The final focused run, workspace lint and formatting pass.

Periodic interpolation still requires the existing adapter's folded columns and
closure constraints. Hermite rows, prescribed controls and multiple right-hand
sides also retain their current adapter. Arbitrary conditioning, extrapolation,
mixed polynomial/rational sentinel payloads, continuous Rhino parity and a
performance improvement are not certified. Leak detection is disabled for the
sanitizer run. Upstream USD, Python binding, OCCT, GUI and `repo.sh`/`prog_test`
harnesses were not run.

```sh
cargo test -p viboceros-smlib --features native --test point_interpolation
```
