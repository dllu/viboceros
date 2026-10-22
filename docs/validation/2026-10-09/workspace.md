# October 9 workspace audit

[Historical checkpoint](../../validation-checkpoint.md)

This is a reproducible regression checkpoint, not a compatibility certificate.
The October 9, 2026 audit tested Contour 3D plane grids and ranges, Section plane cuts and exact mesh tracing, BlockEdit restricted-member close choices and empty-workspace retention, nested BlockEdit contexts and cancellation checkpoints, BlockEdit viewport input and double-click opening, BlockEdit controls and chronology, in-place BlockEdit, BlockResetScale, AddObjectsToBlock, exact affine inversion, ReplaceBlock input and replacement, unique/duplicate block definitions, BlockManager operations and UI, native block group replay, workflow metadata corrections,
block expansion, structural 3DM blocks, object snapping, Block/Insert commands, native instances, the catalog and bounded 3DM
traversal/reflections, STEP extrusion images, certified surface edits, curve Rebuild
previews and prepared surface proof contexts in the worktree based on
`0a204e46` (earlier milestones used `b218b963`, `75fc0291`, `a71ee68b`, `4f513b99`, `6e60921e`, `ed496a1e`, `9e3cbf53`, `0cf4b460`, `7f9f07b6`, `dd130d9e`, `a4add84b`, `c0a405a3`, `368f1d1a` and `35f2b035`), with Rust 1.95.0.
Code and measurement hashes are retained in
[Section provenance](../../section-command-provenance.json),
[restricted block member provenance](../../block-edit-restricted-provenance.json),
[nested block editor provenance](../../block-edit-nested-provenance.json),
[block editor input provenance](../../block-edit-input-provenance.json),
[block editor control provenance](../../block-edit-controls-provenance.json),
[in-place block editing provenance](../../block-edit-provenance.json),
[block scale-reset provenance](../../block-reset-scale-provenance.json),
[block addition provenance](../../block-add-provenance.json),
[replacement input provenance](../../block-replace-input-provenance.json),
[replacement provenance](../../block-replace-provenance.json),
[unique-definition provenance](../../block-unique-provenance.json),
[management provenance](../../block-manager-provenance.json),
[group provenance](../../block-group-provenance.json),
[workflow provenance](../../block-workflow-provenance.json),
[expansion provenance](../../block-explode-provenance.json),
[structural provenance](../../three-dm-structural-provenance.json),
[snap provenance](../../block-snap-provenance.json),
[command provenance](../../block-command-provenance.json),
[instance provenance](../../block-instance-provenance.json),
[block catalog provenance](../../block-definition-provenance.json),
[block audit provenance](../../three-dm-block-audit-provenance.json),
[extrusion provenance](../../step-extrusion-pcurve-provenance.json),
[STEP qualification provenance](../../step-spline-pcurve-provenance.json),
[surface-edit provenance](../../surface-edit-certificate-provenance.json),
[preview provenance](../../curve-rebuild-preview-provenance.json) and
[cross-target provenance](../../retrim-target-cache-provenance.json). Git metadata was
mounted read-only during the earlier captures. Git writes were enabled later on
October 8, allowing these previously validated changes to be committed.

## Commands and results

```sh
cargo test --workspace --release
python3 -m unittest discover -s tools/rhino_oracle -t .
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

The final workspace release run passed 5,602 Rust tests. A preliminary run
caught a command-first source-selection handoff bug; the corrected getter and
Section regression are included in this final run. Clippy, format and diff
checks also passed.

| Package | Passed | Ignored |
| --- | ---: | ---: |
| App | 1,094 | 17 |
| Command | 1,247 | 2 |
| Document | 265 | 5 |
| Drafting | 164 | 6 |
| Geometry | 2,111 | 8 |
| I/O | 212 | 0 |
| Oracle (unit and integration) | 509 | 0 |

The Python suite passed 1,070 tests. The 38 ordinarily ignored Rust tests were
not run in this checkpoint. The September 12 audit of `bd299074` separately
passed six opt-in GPU tests covering 182 renders on NVIDIA GB10 / Vulkan /
driver 610.43.02; that is historical evidence, not a new graphics check.
See [GPU tests](../../gpu-tests.md) for their pixel assertions and limits.

## What this establishes

The current checked cases cover geometry, document transactions, commands,
interchange, CPU viewport behavior, and Python orchestration. The historical
GPU evidence above separately covers production GPU rendering.
The complete workspace test process was observed to finish with exit status 0.
The nonlinear pullback replay adds 1,290 source and image stations at `1e-11`,
with independent recomputation of the final assembled-curve certificates.
Earlier automatic pullback tests check 3,096
paired spatial and surface-image stations at `1e-11`, independently recomputing
the local continuous bounds. Earlier fixed-endpoint tests retain another 1,032
stations. Geometry regressions preserve adjacent-float and subnormal domains,
singular endpoints, signed weight gauges, fixed constraints, and exact rational
linear crossings in both parameter directions.

The newest unique-definition capture covers five workflows and 30 states. The
preceding management capture covers four workflows and 33 states. The preceding
group capture covers 30 workflows, including 16 matching command
cases and 14 SDK-construction cases with eleven diagnostics. The preceding block
capture covers 44 API/command workflows, including 22 matching
command cases and retained SDK diagnostics. The earlier curve capture covers
32 closed numeric recipes, 16 retained pending
point getters, and six cancellations. The preceding direction capture covers 18
recipes with four retained failures and four cancellations. The preceding capture covers a 37-step
preference sequence with raw cancelled queries and option edits. The preceding
capture covers 22 FromMidpoint recipes with three retained
cancellations. The preceding capture covers 15 MarkEnds recipes with two retained
cancellations. The preceding standalone capture covers 17 SubCrv recipes. The
numeric getter/command capture covers 29 recipes,
including one retained cancellation. The signed-length SDK capture covers 20
independent geometry queries. The earlier command capture
covers 14 successful endpoint-input SubCrv recipes.
The preceding face capture retains two rejected coordinate-input attempts.
Other recorded-output
replays are not fresh cross-engine measurements, and tests that merely execute
fixtures with finite output do not establish numerical agreement with Rhino.
Native `Surface.Pullback` and `Surface.Pushup` follow geometric loci and do not
promise these local fitters' normalized-parameter correspondence or fixed UV
endpoint constraints.
Passing these cases does not establish all-command coverage, arbitrary-geometry
epsilon agreement, general curved Booleans, general STEP B-rep interchange,
other graphics backends, or performance parity. Those remain subject to the
implementation boundaries in
the [architecture](../../architecture.md), [file-format](../../file-formats.md), and
[oracle timing](../../oracle/timing.md#timing-interpretation) documentation.
