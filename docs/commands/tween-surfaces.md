# TweenSurfaces

[Command reference](README.md) · [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/tweensurfaces.htm)

`TweenSurfaces` adds intermediate surfaces between two inputs. The current
implementation supports sampled matching (`MatchMethod=SamplePoints`) for
polynomial/rational surfaces with different degrees, counts and parameter domains.
`MatchMethod=Refit` matches degrees and knot layouts across positive rational
nets, including unequal counts. Control matching (`MatchMethod=None`) handles
polynomial and positive rational nets with equal or differing degrees/counts. Sources remain
unchanged.
Start the command, pick the start surface and then the end surface, edit options
or directions while viewing the preview, and press Enter to accept. Escape
cancels any phase. One preselected surface supplies the start; two preselected
surfaces open the preview directly.

```text
TweenSurfaces NumberOfSurfaces=3 MatchMethod=SamplePoints SampleNumber=6
TweenSurfaces NumberOfSurfaces=3 MatchMethod=Refit
TweenSurfaces NumberOfSurfaces=3 MatchMethod=None
TweenSurfaces Sources=<first-id>,<second-id> OutputLayer=StartSrf
TweenSurfaces Sources=<first-id>,<second-id> FlipEndU=Yes SwapEndUV=Yes
```

Explicit `Sources=` fixes script order. `FlipStartU`, `FlipStartV`, `SwapStartUV`,
and their `End` counterparts adjust correspondence without changing originals.
Swap applies before flips. Source picks retain click order, independently of
document insertion order or source groups. The same surface can be picked twice.
Three end-surface corner controls are clickable in every viewport: `U/V` toggles
U/V swapping, `U` reverses U, and `V` reverses V. Swap retains the U/V reversal
flags in their existing slots, following measured repeated native clicks; it does
not transpose those flags. A swap after a reversal can therefore move the origin. The controls follow the
current end-surface parameterization after each edit. Start-surface corners and
the opposite end corner do not act, matching the measured native behavior.
Typed direction options remain available for either source.

Options accept `Name=value`, `Name value`, or an option name followed by its
value at the next prompt. Invalid edits retain the previous valid preview. Enter
at a value prompt keeps the current value and returns to the options phase.
View and display commands remain available during preview. Source geometry, attributes,
groups, tolerance and current-layer changes invalidate the cached result.
Preview preparation is readonly; unchanged option edits reuse the current scene.
Acceptance reuses the prepared geometry in one transaction.

The initial matching default is `SamplePoints` with `SampleNumber=10`, following
the owned fresh native settings. `SampleNumber` specifies divisions per UV axis;
there are one more stations than divisions. Both sources are evaluated at matching
normalized UV positions, blended, then interpolated as a non-rational tensor
surface with degree up to three. Knot placement uses mean chord distances on the
blended grid. Domains follow those physical chord sums and can differ between
outputs. This interpolates the grid; it does not certify a continuous exact
blend of the source surfaces. Sampling is bounded to `2..=255` divisions per axis,
matching the existing 256-station dense tensor solve limit. Source trims are not
sampled. Native sample counts above this local limit remain unsupported.

Count, matching method and sample count remember their last successful
acceptance. Cancelling the options phase discards edits to those fields. The
sample count is saved even when acceptance occurs in Refit or control matching,
so returning to SamplePoints recovers the last edited count. `OutputLayer` edits
instead take effect immediately on reaching the options phase and survive
cancellation. Cancelling during source selection does not save an inline layer
preset. Invalid input changes neither the staged options nor remembered values.
Undo/Redo does not change preferences. They belong to the application's command
registry, are shared across its documents, and reset in a fresh registry; restart
persistence remains unverified. Source IDs and direction flags are per invocation.
Use explicit options in deterministic scripts.

Counts default to one and are bounded at 4,096, with an aggregate million-control
limit. Outputs exclude sources, at fractions `i / (number + 1)`. When control
degrees and counts agree in both axes, matching retains the first source's
knots and domains even when the second source's knot layout differs.
Compatible rational matching retains the first weights and scales each
control displacement by `sqrt(end_weight / start_weight)`, following measured
native data. This can extrapolate control locations.

When any degree or count differs, control matching rebuilds both prepared
sources to the maximum degree and count in each axis. Target knots are uniform
with unit spans. Target Greville fractions map to equal arc-length fractions on
each source's two midpoint isocurves. Evaluating that tensor parameter grid and
solving its two collocation systems produces non-rational control nets. Both
sources are rebuilt again before each successive output, following the measured
native drift rather than reusing the first prepared nets. In the curved three-output
case, the second output differs from a single-rebuild blend by about `0.00835`
at a control point. Original sources remain
unchanged. Rebuilding uses the internal default arc-length tolerance and at most
256 controls per axis; solves normalize coordinates and check pivots, finite
solutions and backward residuals. This approximates the sources and does not
certify preservation of their continuous loci.

Refit prepares a common tensor basis by clamping, elevating degrees and inserting
the union of normalized interior knots with their maximum multiplicities. It then
uses the measured rational control-displacement policy above. Output UV domains
retain the end source's native intervals. This exact basis matching covers the
recorded inputs; it does not establish every native adaptive fitting policy.
The same control and output limits apply before preparation and insertion.

`OutputLayer=CurrentLayer` uses fresh default attributes and no groups.
`StartSrf` and `EndSrf` copy the corresponding attributes and group memberships.
Geometry-root text is not copied. Outputs finish unselected. Preselected sources
retain their selection through acceptance, cancellation and history; sources
picked after starting finish unselected. One Undo removes every output; Redo
restores the accepted result. Construction
is staged before insertion, and failures preserve geometry and history.

The [complete capture](../../tools/rhino_oracle/observations/tween_surfaces_command.json)
contains 22 successful owned Rhino recipes on private Xvfb under
`VibocerosOracleTweenSurfacesVerified20261008`. It retains full NURBS definitions,
81 stations per surface, properties/groups, source identity, command events
and independent Undo/Redo, plus public sampling SDK results. Eight compatible
control recipes replay degree/counts, knots, controls and weights at `1e-6` for
positions and `1e-12` for knots/weights: planes, quadratic/rational nets, different
domains, swapped rational order and output layers. Independent regressions
check extreme finite coordinates, source purity, resource rejection and history.
See [provenance](../tween-surfaces-provenance.json).

Earlier commands that changed matching options left additional source-shaped
copies: `Refit` four and `SamplePoints` two in that capture, alongside requested
tweens. The public sampling SDK returns
just the requested outputs. Both raw records and the initial investigation are
retained without normalizing these differences. Native preview appearance and
performance parity remain unverified.
The registered command rejects unsupported methods. Trimmed single-face input
uses its underlying surface; trim correspondence is not implemented.

The [sampling follow-up](../../tools/rhino_oracle/observations/tween_surfaces_sampling.json)
adds 20 successful owned commands under `VibocerosOracleTweenSampling20261008`
with sample counts 2, 3 and 6. All requested command surfaces exactly equal the
public sampling SDK definitions; this run keeps the matching mode active and
produces no extra source-shaped copies. Kernel replay checks all 20 output nets
at `1e-7` for controls and `1e-8` for knots, plus all 22 earlier SDK outputs.
Command replay compares 81 UV-normalized witnesses per source/output at `1e-7`,
identity, metadata/groups and independent history. Cases cover planes, warped
and rational tensors, unequal counts/degrees/domains and source output layers.
The app regression checks that method, sample count and output count survive
command-first selection and Undo/Redo. Independent kernel checks require
analytic plane agreement at `1e-12` and reject excessive work before allocation.
See [sampling provenance](../tween-sampling-provenance.json).

The [Refit follow-up](../../tools/rhino_oracle/observations/tween_surfaces_refit.json)
adds 24 successful owned commands under `VibocerosOracleTweenRefit20261008`.
Each recipe initializes native options in the owned document, retains that
initialization record, deletes its outputs and clears history, then starts a new
command and accepts without option changes. Every accepted command produces
exactly the requested one, two or three surfaces. Native initialization still
creates extra objects; this isolates those diagnostics from accepted geometry.

Kernel replay checks all 24 accepted nets and six earlier requested Refit nets
at `1e-7` for controls and `1e-12` for knots/weights. Cases include polynomial/
rational inputs, unequal degrees/counts, swapped order, differing domains and
source-layer outputs. Command replay compares 81 UV-normalized witnesses per
source/output at `1e-7`, complete identity/properties/groups and independent
history. An independent polynomial test checks normalized parameter agreement
at `1e-12` with different degrees, U/V knot sites and multiplicities, while
preserving original sources. App tests exercise Refit options, source picking,
unequal degrees and Undo/Redo. See [Refit provenance](../tween-refit-provenance.json).
The local workflow creates the requested outputs; native option-change copies
remain unimplemented; remembered field lifetimes are described above.

The [interaction capture](../../tools/rhino_oracle/observations/tween_surfaces_interaction.json)
adds nine owned private-Xvfb recipes under
`VibocerosOracleTweenInteraction20261008`: three accepted commands and six
cancellations with zero, one or two preselected sources. Complete snapshots and
command traces retain selection and independent history. The one-preselected
macro selects that same surface as the end; native accepts it and produces copies.
App replay matches all nine outcomes, including 81 normalized witnesses per
source/output at `1e-7` and exact source/history restoration. Independent tests
check reversed pick order, readonly/cached previews, number and direction edits,
invalid values, cancellation, stale input and unrelated transaction preservation.
See [interaction provenance](../tween-interaction-provenance.json).

A private-Xvfb inspection of the wgpu/egui app checks two ordered viewport picks,
three-result preview, `FlipEndU`, Ghosted display, invalid count retention,
acceptance, Undo/Redo and cancellation. The preview viewport shows five objects
while the live layer pane still counts two sources; after acceptance/Redo it
counts five. These local images do not compare native pixels.

![Readonly surface tween preview with a direction edit](../images/tween-surfaces-preview-ghosted.png)

![Accepted surface tween after Undo and Redo](../images/tween-surfaces-redo-ghosted.png)

The [option-lifetime capture](../../tools/rhino_oracle/observations/tween_surfaces_options.json)
runs a 29-step owned sequence under `VibocerosOracleTweenOptionsVerified20261008`.
It retains initial/final native option prompts, accepted geometry, cancellations
and independent history. App replay compares every next-invocation default,
including the different layer/count save rules, rejected values and method
changes. An [eight-step follow-up](../../tools/rhino_oracle/observations/tween_surfaces_sample_memory.json)
under `VibocerosOracleTweenSampleMemory20261008` confirms that a sample count
edited before accepted Refit is revealed when returning to SamplePoints.
Command tests cover independent registries, reused documents, per-invocation
source/direction state and failed invocation admission. See
[option provenance](../tween-options-provenance.json).

The cancelled Refit edit in that sequence leaves two native source-shaped copies,
while discarding the method change for the next invocation. Those objects remain
an explicit geometry discrepancy; local readonly previews discard all staged
geometry on cancellation. The older initial investigation is retained separately.
Neither sequence establishes preference persistence across application restarts.

The [corner capture](../../tools/rhino_oracle/observations/tween_surfaces_corners.json)
adds nine successful owned outcomes under `VibocerosOracleTweenCornerVerifiedFinal20261008`:
one baseline and eight real mouse clicks. Public viewport matrices and
WorldToClient projections calibrate every target; recorded mouse pixels agree
with the integer aim. App replay compares 81 normalized witnesses per
source/output at `1e-7` and independent history for all outcomes. Initial coordinate
and recovered mouse investigations remain separate records; the final driver
completes without manual recovery. See [corner provenance](../tween-corners-provenance.json).

Corner positions are cached alongside prepared geometry. Pixel hit testing is
independent of snaps and uses a nine-pixel radius with nearest-distance ordering.
Controls hide during source selection and value questions; a failed preview
retains them so correspondence can be edited. Independent tests cover axis
composition, source purity, clipped/missed hits and all three display modes.
A private-Xvfb inspection checks U, moved-origin swap and V clicks, Ghosted mode,
readonly preview, acceptance and Undo/Redo on the production wgpu/egui path.
The repeated sequences below extend the single-click evidence. Periodic/singular
and overlapping projected controls, native labels/colors and continuous pixel
agreement remain unverified.

![End-surface corner controls after direction edits in a readonly preview](../images/tween-surfaces-corner-controls.png)

The [repeated-click capture](../../tools/rhino_oracle/observations/tween_surfaces_corner_sequences.json)
adds 14 successful owned commands and 28 real mouse clicks under
`VibocerosOracleTweenCornerSequencesVerified20261008`. Every successive click is
calibrated against a public viewport matrix and released only after a native
mouse-event acknowledgement. It covers repeated swaps, reversals followed by
swaps, returns to the original orientation, inactive-corner clicks after reversal,
both reversals and three-click combinations. A 14-case app replay finds the
current control at each fixed world corner and compares 81 normalized witnesses
per source/output at `1e-7`, along with exact source restoration and Undo/Redo.
This corrected the previous implementation's reversal-flag transposition on swap.
Some measured outputs collapse in projection or to a spatial curve; they remain
represented rather than being silently discarded.

Queued corner edits now revalidate source geometry, attributes/groups, tolerance
and current layer before changing state. Regressions cover stale sources and
failed preparation after a valid preview: the old scene is discarded as one unit,
acceptance stays blocked, and a supported edit can recover. The initial stalled
timer-driver diagnostics are retained; the final acknowledged-click run completed
without manual recovery. See [sequence provenance](../tween-sequences-provenance.json).
This extends the captured open warped bilinear sources; general singular or
periodic source corners and coincident controls remain unverified.

The [initial control capture](../../tools/rhino_oracle/observations/tween_surfaces_control_initial.json)
contains 24 owned native commands with one or two outputs, including unequal
polynomial/rational nets, swapped order, single-axis count/degree differences,
cubic and curved controls, and compatible nets with differing knots.
A [14-case follow-up](../../tools/rhino_oracle/observations/tween_surfaces_control_followup.json)
checks three outputs and equal-count degree changes. Both ran on private Xvfb
and retain full surfaces, 81 normalized stations, attributes/groups, source
identity and independent history. Kernel and command replay check controls and
positions at `1e-6`, knots/weights at `1e-12`, and unchanged sources. Public
Surface.Rebuild records independently check initial preparation. App replay
checks readonly curved-net preview, three-output acceptance and Undo/Redo.
Separate 24-case blend and two-case control-row diagnostics retain rejected
preparation hypotheses and native option-change copies. See
[control provenance](../tween-control-provenance.json) for immutable producer and
capture hashes. Periodic/general singular inputs, extreme rational gauges and
arbitrary high-degree conditioning remain unverified.

The JSON/Python oracle accepts `surface_tween_sampled_geometry` with
`start_surface`, `end_surface`, `number` and `sample_number`. It returns full
`surfaces` definitions in both engines and shares the same resource limits.

```sh
cargo test --release -p viboceros-geometry surface_tween
cargo test --release -p viboceros-command tween_surfaces
cargo test --release --bin viboceros tween_surfaces
python3 -m unittest tools.rhino_oracle.test_tween_surfaces tools.rhino_oracle.test_tween_surfaces_sampling tools.rhino_oracle.test_tween_surfaces_refit tools.rhino_oracle.test_tween_surfaces_interaction tools.rhino_oracle.test_tween_surfaces_options tools.rhino_oracle.test_tween_surfaces_corners tools.rhino_oracle.test_tween_corner_sequences tools.rhino_oracle.test_tween_surfaces_control
```
