# Native MakePeriodic smoothing

With `native-smlib`, `MakePeriodic Smooth=Yes` and the Python oracle's curve and
surface conversion probes delegate periodic collocation to usd-brep. The shared
core checks closure and control counts, preserves active knots, selects the
first in-domain periodic Greville phase, evaluates normalized homogeneous
source targets, dehomogenizes the result and repeats the exact seam controls.
`Smooth=No` keeps its direct cyclic control and knot edits.

The existing interpolation bridge evaluates zeroth derivatives using
`N_BasisDerivs` and folds columns modulo the unique control count. Small systems
use native full partial-pivot LU. Larger systems reuse the existing alternate-end
cyclic ordering and banded LU when `unique_count > 4 * degree + 1`, with a band at
most `4 * degree + 1` entries wide. MakePeriodic's Greville phase can start up to
`degree - 1` controls later than the point-grid phase. The shared core supplies
that starting control index so native columns align with the actual stations.
Exact support, factorization, back-solving, finite-output or residual failures
retry the native full pivoted producer; other basis/span and input errors propagate.
The full path applies row pivots before scalar homogeneous-channel back solving,
avoiding the real-vector pivot solver's small-RHS skip. Exact binary parameter
charts, native span checks, finite output and normalized input residual checks
remain mandatory. Nonuniform stations can start strictly inside the active
domain. They retain their own Greville phase rather than the uniform Rebuild
column shift. An appended domain endpoint describes the interval and
does not add a repeated target constraint.

The portable implementation remains available for builds without usd-brep,
degrees above 17 and more than 1,000 unique controls. A requested surface axis
outside those bounds selects the shared portable surface producer. Supported
native failures propagate. Surface conversion currently factors each control
row separately; batching and tensor performance remain pending.

The native regressions expand independent quadratic rational basis coefficients
at the preserved stations, including signed common weight gauges from `1e-280`
to `1e280` and a nonzero `1e-280` Z channel. A separable rational tensor checks
independent two-axis constraints. Other tests check all supported degrees,
nonuniform Greville phases far from the domain start, complete portable nets,
seam derivatives, exact repeated controls, no-op conversion, closure admission
and the higher-degree portable fallback. Larger degree-2/3/7/11/17 systems check
both uniform and nonuniform nets at 64 and 256 unique controls. An independent
64-row cyclic permutation, with every diagonal zero, exercises the pivoted
retry and tiny channels. The existing command regressions check
staged edits, attributes, groups, selection, retained inputs and undo/redo.

```sh
cargo test --release -p viboceros-smlib --features native --test make_periodic
cargo test --release -p viboceros-command --features native-smlib make_periodic
```

The three retained request sets `curve_make_periodic`,
`curve_make_periodic_degrees` and `surface_make_periodic` contain 23 conversions.
Their pre-change producer is retained as a portable-behavior comparison, not a
fresh Rhino observation. All 23 complete definitions match at absolute `1e-9`,
relative `1e-12`, with maximum difference `8.89e-15`. Results are recorded in
[the replay artifact](smlib-make-periodic-comparison.json).

The [diagnostic timing artifact](smlib-make-periodic-performance.json) records
five requests with five timed conversions and an untimed warmup per operation.
All 12 uniform/nonuniform degree-three/eleven definitions match the portable
producer. On this ARM64 machine, 512 unique controls with uniform knots take
median `14.65 ms` portable versus `0.205 ms` native at degree three (about 71×),
and `14.79 ms` versus `0.571 ms` at degree eleven (about 26×). Smaller 32-control
cubic systems are near parity. Serialization is excluded; these are producer
diagnostics, not Rhino timings or a guarantee for arbitrary conditioning.
Reproduce with two saved oracle executables:

```sh
python3 tools/usd_brep/benchmark_make_periodic.py \
  --baseline /path/to/portable-oracle --native /path/to/native-oracle \
  --output /tmp/make-periodic-performance.json
```

Fresh Rhino capture remains blocked by Wine licensing startup. These station
and definition checks do not establish arbitrary conditioning, continuous Rhino
parity or a speed advantage over Rhino. Smoothing intentionally changes the
curve between interpolation stations.
