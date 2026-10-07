# Validation checkpoint

[Architecture and status](architecture.md) · [Rhino oracle](oracle.md)

This is a reproducible regression checkpoint, not a compatibility certificate.
The October 6, 2026 audit tested code at `3cbe2b1b` with Rust 1.95.0 after
[automatic certified pullback branches](automatic-surface-pullbacks.md), exact
linear knot crossings, and one-dimensional surface restrictions.

## Commands and results

```sh
cargo test --workspace --release
python3 -m unittest discover -s tools/rhino_oracle -t .
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_pullback_linear_certified.json --timeout 600 --output /tmp/automatic-pullbacks-local.json
```

All commands completed successfully. The ordinary Rust suite passed 5,085 tests:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| App | 937 | 17 |
| Command | 1,135 | 2 |
| Document | 183 | 5 |
| Drafting | 157 | 6 |
| Geometry | 2,000 | 8 |
| I/O | 195 | 0 |
| Oracle | 478 | 0 |

The Python suite passed 861 tests. The 38 ordinarily ignored Rust tests were
not run in this checkpoint. The September 12 audit of `bd299074` separately
passed six opt-in GPU tests covering 182 renders on NVIDIA GB10 / Vulkan /
driver 610.43.02; that is historical evidence, not a new graphics check.
See [GPU tests](gpu-tests.md) for their pixel assertions and limits.

The standalone Python pullback run certified all 24 new native sources without
endpoint constraints, returning two UV controls and each original spatial
parameter domain. All reported continuous bounds were below `1e-12`, with a
maximum of `8.86e-16`. The run used 16 iterations per source, retaining the
successful bound without recomputing its certificate.

A fresh 24-recipe Rhino 8.32.26160.13001 capture ran on private Xvfb with settings
scheme `VibocerosOracleLinearPullbackFinal_20261006`. Its
[raw records](../tools/rhino_oracle/observations/surface_pullback_linear.json)
retain full source definitions, source purity, 129 stations per recipe, 20
successful native pullbacks, four native failures, and a parameter-speed
discrepancy for both directions of the swapped two-pole surface. The local
certificates and native geometric fitting have different contracts; see
[provenance](automatic-surface-pullback-provenance.json) and
[timing details](automatic-surface-pullback-performance.json).

The earlier eight-recipe capture also ran on private Xvfb with settings scheme
`VibocerosOraclePullbackEndpointsVerify_20261006`, using the eight
[`surface_pullback_endpoints` recipes](../tools/rhino_oracle/fixtures/surface_pullback_endpoints.json).
Its complete output was identical to the
[retained capture](../tools/rhino_oracle/observations/surface_pullback_endpoints.json),
including the planar parameter-speed differences and independent endpoint
witnesses. Both files had SHA-256
`71593c8aa90b81679ed1a196a5a25fd9738ecd9febdc71cd237934a3e0e436a0`.
The owned Rhino, Python worker, and Xvfb processes exited after the capture.

## What this establishes

The current checked cases cover geometry, document transactions, commands,
interchange, CPU viewport behavior, and Python orchestration. The historical
GPU evidence above separately covers production GPU rendering.
The complete workspace test process was observed to finish with exit status 0.
The new pullback tests replay full native source definitions and check 3,096
paired spatial and surface-image stations at `1e-11`, independently recomputing
the local continuous bounds. Earlier fixed-endpoint tests retain another 1,032
stations. Geometry regressions preserve adjacent-float and subnormal domains,
singular endpoints, signed weight gauges, fixed constraints, and exact rational
linear crossings in both parameter directions.

The new native capture covers 24 SDK recipes. Other recorded-output
replays are not fresh cross-engine measurements, and tests that merely execute
fixtures with finite output do not establish numerical agreement with Rhino.
Native `Surface.Pullback` follows a geometric locus and does not promise the
local fitter's normalized-parameter correspondence or fixed UV endpoints.
Passing these cases does not establish all-command coverage, arbitrary-geometry
epsilon agreement, general curved Booleans, general STEP B-rep interchange,
other graphics backends, or performance parity. Those remain subject to the
implementation boundaries in
the [architecture](architecture.md), [file-format](file-formats.md), and
[oracle timing](oracle.md#timing-interpretation) documentation.
