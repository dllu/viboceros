# Validation checkpoint

[Architecture and status](architecture.md) · [Rhino oracle](oracle.md)

This is a reproducible regression checkpoint, not a compatibility certificate.
The September 12, 2026 audit tested commit
`bd299074dd3b7832884a20283962083d00813aee` with Rust 1.95.0 after the mesh
radial-order and selected-vertex Unweld refactors.

## Commands and results

```sh
cargo test --workspace
python3 -m unittest discover -s tools/rhino_oracle -t .
cargo test -p viboceros viewport::raster_tests -- --ignored --nocapture
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

All commands completed successfully. The ordinary Rust suite passed 2,298 tests:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| App | 317 | 7 |
| Command | 554 | 2 |
| Document | 140 | 5 |
| Drafting | 27 | 0 |
| Geometry | 942 | 3 |
| I/O | 84 | 0 |
| Oracle | 234 | 0 |

The Python suite passed 165 tests. Six of the 17 ordinarily ignored Rust tests
are GPU tests; the explicit GPU run passed all six, covering 182 renders on
NVIDIA GB10 / Vulkan / driver 610.43.02. See [GPU tests](gpu-tests.md) for
their pixel-level assertions and limits. The other 11 ignored tests are manual
timing diagnostics and were not run in this checkpoint.

## What this establishes

The checked cases cover geometry, document transactions, commands, interchange,
CPU viewport behavior, Python orchestration, and production GPU rendering.
The oracle suite includes the 165 recorded mesh-edit cases documented in
[mesh topology](mesh-topology.md). The complete workspace process, including
the long-running sweep fixture test, was observed to finish with exit status 0.

No live Rhino session was launched for this checkpoint. Recorded-output replay
is not a fresh cross-engine measurement, and tests that merely execute fixtures
with finite output do not establish numerical agreement with Rhino.
Passing these cases does not establish all-command coverage, arbitrary-geometry
epsilon agreement, general STEP B-rep interchange, other graphics backends, or
performance parity. Those remain subject to the implementation boundaries in
the [architecture](architecture.md), [file-format](file-formats.md), and
[oracle timing](oracle.md#timing-interpretation) documentation.
