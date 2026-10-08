# Rebuild surfaces

[Command reference](README.md) · [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/rebuild.htm)

`Rebuild` reconstructs selected single-face surfaces with independent U/V degrees
and control counts. Curves retain their existing Rebuild options. With selected
surfaces, bare `Rebuild` opens an options prompt. Starting it without selection
asks for surfaces first; Enter then opens options. A complete inline invocation
accepts after picking, or runs directly on preselected surfaces.

```text
Rebuild UPointCount=5 VPointCount=4 UDegree=3 VDegree=2
Rebuild UPointCount=7 VPointCount=5 UDegree=3 VDegree=2 DeleteInput=No OutputLayer=Current ReTrim=No
```

Fresh defaults are 10 controls and degree 3 in each direction, `DeleteInput=Yes`,
`OutputLayer=Input` and `ReTrim=Yes`. Degrees accept 1 through 11; counts must
exceed their degree and are locally bounded at 256 per axis. Aggregate preparation
is limited to one million controls. Scripted degree increases raise the associated
count to at least `degree + 1`. A count below the current degree minimum is
rejected, retaining the count. Edits apply in token order, so lowering the degree
first can admit a later smaller count. This follows measured native script
getters; the native dialog's automatic degree lowering remains unimplemented.

At the options prompt, use `Name=value`, `Name value`, or an option name followed
by a value at the next prompt. Enter at a value prompt keeps the value and
returns to options. Enter in options accepts; Escape or Cancel ends the command.
Every valid edit is remembered immediately, including counts, degrees, deletion,
layer and ReTrim, and survives cancellation and Undo/Redo. Invalid edits change
neither preferences nor geometry. Preferences belong to the command registry,
are shared by its documents and reset in a new registry. Application restart
persistence and interaction with curve Rebuild preferences remain unverified.

The shared kernel samples two midpoint isocurves by arc length, maps uniform
Greville fractions to their source parameters, evaluates a tensor grid and solves
two collocation systems. The result is non-rational with uniform unit-span knots.
Solves normalize coordinates and check pivots, finite controls and backward
residuals. This approximates the original surface; it does not certify continuous
locus preservation. Positive source weights are required.

Replacement retains source identity and copies its attributes, including name,
color and attribute user text. Output groups are cleared. `OutputLayer=Current`
moves a replacement or copy onto the current layer. Copying preserves the
original geometry and groups. Outputs finish unselected; one Undo restores the
originals and Redo restores the accepted objects. All geometry is prepared before
document edits.

Untrimmed surfaces support either ReTrim value. `ReTrim=No` intentionally rebuilds
a trimmed single face's full underlying surface. `ReTrim=Yes` on trimmed input
remains unsupported, pending physical trim projection onto the changed surface.
Polysurfaces, periodic/general singular inputs, mixed curve/surface batches,
native preview, restart persistence and performance parity remain unresolved.
Geometry-root user text is absent from the captured native inputs, so its
replacement lifetime is not established by this evidence.

The [12-command capture](../../tools/rhino_oracle/observations/surface_rebuild.json)
ran on private Xvfb with polynomial/rational inputs and all copy/replacement and
input/current-layer combinations. It retains complete controls, 81 stations,
attributes/groups, source identity and independent history. Native command
definitions exactly equal their corresponding public Surface.Rebuild results.
Kernel and command replays use `1e-6` for positions and exact knots/weights.
A separate [12-query SDK capture](../../tools/rhino_oracle/observations/surface_rebuild_geometry.json)
checks additional reductions and asymmetric counts/degrees through the JSON API.
Its initial local audit exposed two rational endpoint-distance failures from a
one-step floating-point overshoot; endpoint parameters now use exact source
domain boundaries, with fractions formed before length multiplication.
The initial six wrong-option commands remain separate diagnostics. See
[provenance](../surface-rebuild-provenance.json) for producer and capture hashes.

The [25-step option capture](../../tools/rhino_oracle/observations/surface_rebuild_options.json)
and [nine-step ordered follow-up](../../tools/rhino_oracle/observations/surface_rebuild_option_followup.json)
ran under fresh private Xvfb settings schemes. Full native prompts distinguish
immediate memory, rejected counts and degree-driven count growth. App replay
checks every initial/final state, cancellations, accepted controls and independent
history. Separate regressions cover numeric value questions, unchanged invalid
edits, registry isolation and alias access. See
[option provenance](../surface-rebuild-options-provenance.json).

The oracle operation `surface_rebuild_geometry` accepts `surface`, `point_count`
and `degree`, returning the complete `surface` definition in either engine.

```sh
cargo test --release -p viboceros-geometry surface_rebuild
cargo test --release -p viboceros-command surface_rebuild
python3 -m unittest tools.rhino_oracle.test_surface_rebuild tools.rhino_oracle.test_surface_rebuild_options
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/surface_rebuild_geometry.json --scheme VibocerosOracleSurfaceRebuildSDK --absolute-epsilon 1e-6 --relative-epsilon 1e-10
```
