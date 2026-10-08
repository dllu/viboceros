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

`ReTrim=No` rebuilds a single face's full underlying surface with natural boundaries
and uniform output domains. `ReTrim=Yes` transfers the original physical edges
onto the rebuilt surface and retains the source UV domains, including natural
boundaries. Holes, face orientation and shared endpoint parameters are retained.
This uses physical projection: copying normalized UV coordinates gives incorrect
boundaries when the source parameter speed changes.

The kernel tries a certified exact pullback first. Otherwise it fits bounded
closest-point projections in normalized UV, with a fit tolerance scaled by
polynomial derivative-control bounds. Projection searches and interpolation
error checks are sampled and do not prove a global closest point or continuous
projection error. New spatial edges have independent continuous certificates
against the new UV trims, and the assembled B-rep must pass ordinary validation.
Neither original spatial edges nor component tolerances are silently retained
when inconsistent with the rebuilt surface. Different native and local fitting
representations remain visible in the records.

Polysurfaces, seam/singular trims, rational target surfaces, mixed curve/surface batches,
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

The [eight-case retrim capture](../../tools/rhino_oracle/observations/surface_rebuild_retrim.json)
and [six-case follow-up](../../tools/rhino_oracle/observations/surface_rebuild_retrim_followup.json)
ran on private Xvfb. They retain complete source/output B-reps, 33 stations per
edge, 81 surface stations and independent history for rectangular planar/warped
cuts, a planar hole, nonuniform control spacing, curved/rational sources and
natural boundaries. Every native retrim command equals public
`Brep.CreateTrimmedSurface`; the UV-copy candidate disagrees materially on the
nonuniform source. Local command replay compares target controls at `1e-6` and
native edge distance witnesses at `2e-6`, plus source purity and Undo/Redo.
The native circular hole has a fitted polynomial representation; local exact
rational representations are retained. An analytic offset-plane test checks
area, holes, orientation and all new trim/edge certificates. App testing checks
readonly option edits, accepted holes and history. See
[retrim provenance](../surface-rebuild-retrim-provenance.json).
The initial default 10×10 circular-hole regression exhausted the ordinary
knot-crossing certificate. A global affine control-net certificate now proves
that case continuously while retaining rational circle controls; it also rejects
incorrect translated images and mixed-sign UV proposals.

The oracle operation `surface_rebuild_geometry` accepts `surface`, `point_count`
and `degree`, returning the complete `surface` definition in either engine.
`brep_retrim_geometry` accepts a closed-loop `fixture` and target `surface`,
returning complete `brep` topology, definitions and geometry witnesses. It exposes
trim transfer separately from rebuilding for instrumentation.
Two [public SDK queries](../../tools/rhino_oracle/observations/brep_retrim_geometry.json)
project a paraboloid annulus onto parallel polynomial planes. API replay checks
the two loops, projected vertices and native edge distance witnesses at `2e-6`.

```sh
cargo test --release -p viboceros-geometry surface_rebuild
cargo test --release -p viboceros-command surface_rebuild
python3 -m unittest tools.rhino_oracle.test_surface_rebuild tools.rhino_oracle.test_surface_rebuild_options tools.rhino_oracle.test_surface_rebuild_retrim
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/surface_rebuild_geometry.json --scheme VibocerosOracleSurfaceRebuildSDK --absolute-epsilon 1e-6 --relative-epsilon 1e-10
```
