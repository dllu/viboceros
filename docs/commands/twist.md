# Twist

[Command reference](README.md) · [Transforms](transforms.md)

Select objects, run `Twist`, pick the axis start and end, then enter a signed
angle in degrees. Alternatively pick two reference directions off the axis.
Starting without a selection first prompts for objects; Enter accepts that
selection. Coordinate input, object snaps and point filters use the common
point-input path. For example:

```text
Point 2,1,5
SelAll
Twist 0,0,0 0,0,10 90
Undo
Twist 0,0,0 0,0,10 1,0,0 0,1,0 Copy=Yes
```

| Option | Behavior |
| --- | --- |
| `Copy=Yes/No` | Keep sources and create copies, or replace their geometry. Interactive Copy accepts more angles from the original sources; Enter or Escape keeps accepted copies and finishes the batch. One Undo reverses the batch. |
| `Rigid=Yes/No` | Rotate and translate each object without deforming it. Selected members of the same top group use their combined bounds center and one placement. |
| `Infinite=Yes/No` | Use a constant angular rate beyond both axis ends, or a finite cubic blend. |
| `PreserveStructure=Yes/No` | Move existing Euclidean NURBS controls while keeping degree, weights, knots and domains. Multi-face B-reps always use fitting. |

During interactive input, options accept `Name=Yes/No` or a named prompt followed
by Yes/No. Complete scripted invocations use `Name=Yes/No`. A zero angle
makes no changes, including with Copy enabled. Invalid axes, references, or
unrepresentable geometry fail without changing any source. Names, object colors,
layers and group topology survive; copied objects have independent group
records and remain unselected, while their sources retain selection. The
shared RememberCopyOptions setting applies to Copy.

Rigid, Infinite and PreserveStructure remember their last successful values,
including zero-angle runs. Cancelled option edits are discarded. Accepted Copy
placements remember their choices even when Escape ends the batch; edits after
the last placement are discarded. Undo, Redo and a new document retain these
application preferences. Independent application instances start with No.
With preselected sources and Copy=No, Undo restores source selection; Redo releases
it unless the source was explicitly selected again after Undo. Unrelated selection
survives.

## Live preview

After choosing the first reference direction, mouse motion draws temporary
object-colored wires or points over the selected sources. Shaded and ghosted
views retain the original faces and their object or mesh colors. The temporary
wires draw over those faces while obeying the viewport's near/far clipping.
An axial reference pick retains the last valid preview and keeps the prompt
active. Moving through successive turns accumulates the mouse angle; a typed
number uses its explicit signed angle.

The measured standalone curve preview elevates to at least cubic degree and
maps controls, including when PreserveStructure is enabled. Low-degree
polynomial surfaces and B-rep edges use a quick cubic interpolant of mapped
Greville samples. PreserveStructure keeps the original surface controls and
single-face B-rep edge controls. These temporary approximations skip adaptive
fitting; accepting the command follows its final-geometry policy.
Mesh wires map vertices directly. Rigid previews reuse source display geometry
with an affine placement. Prepared cages and trim intervals are shared across
the viewports; unchanged angles and stationary views reuse their cached display.
Copy starts another angle/reference prompt after each placement, clearing the
temporary preview; Escape keeps accepted copies.

## Geometry and measured compatibility

For axis length `L` and axial coordinate `z`, finite twists rotate by
`angle * t² * (3 - 2t)`, where `t = clamp(z/L, 0, 1)`. Infinite twists use
`angle * z/L`. The axis start is unchanged and the end receives the full angle.
The public point map uses OpenNURBS cardinal-angle cleanup; the kernel's general
rotation entry point continues to preserve small angles.

Rigid placement is independently inferred from public outputs: take forward
world-axis differences at the bounds center. Nearly orthogonal sampled frames
retain their first two directions; other frames average each unit direction
with its unit dual normal before orthonormalizing. Five supplementary small-angle
command witnesses test both sides of the shared frame-validity boundary;
see the [Bend command provenance](../bend-command-provenance.json).
The measured difference step is
`sqrt(max(abs(center coordinates)) * 1.490116119385e-8 + 2^-32)`.
No proprietary code was inspected.

Fitted command geometry uses absolute tolerance `max(document tolerance, 1e-5)`.
Retained command captures show identical fitted results from `1e-5` down to
`1e-11`; public SDK captures confirm the floor through `1e-9`. Larger tolerances
produce coarser results. Native curve
samples differ from the exact point map by approximately `5–6e-6` in these
cases, despite tighter document settings. Points, meshes, rigid placements and
preserved controls keep their existing policies. The kernel's explicit morph
fitters continue to honor their requested tolerance or return an error.

Private Xvfb captures from Rhino **8.32.26160.13001** retain:

- [14 public SDK point maps](../../tools/rhino_oracle/fixtures/twist_points.json),
  compared at absolute coordinate epsilon `1e-11`.
- [36 actual commands](../../tools/rhino_oracle/fixtures/twist_command.json):
  signed angles, Points, Line, cubic NURBS, a surface, a solid box, a colored
  quad mesh, and all four options. Curve and surface samples use `2e-5`, allowing
  each fitted result its `1e-5` document tolerance; preserved structure uses
  `1e-11`, rigid placements `1e-7`, and native float mesh positions `1e-6`.
  Box face correspondence is established from the original patches before
  comparing their deformed sample grids; solid incidence is checked too.
- [12 rigid/zero-angle cases](../../tools/rhino_oracle/fixtures/twist_rigid_command.json):
  reversed, longer and spatial axes, translated objects, different document
  tolerances and ungrouped points.
- [Four repeated Copy batches](../../tools/rhino_oracle/fixtures/twist_repeat_command.json),
  retaining source selection, independent groups, zero-angle suppression,
  Undo and Redo.
- [12 tight-tolerance commands](../../tools/rhino_oracle/fixtures/twist_tight_command.json)
  and [18 fitting-policy cases](../../tools/rhino_oracle/fixtures/twist_fitting_command.json),
  covering finite/Infinite curves, a surface and a solid box. Sample comparisons
  use `2 * max(document tolerance, 1e-5)`, allowing independent fit errors.
  Public SDK curve and single-face geometry records equal command outputs;
  SDK box caps can retain a different degree/control layout.
- [A 39-step preference workflow](../../tools/rhino_oracle/fixtures/twist_options_command.json)
  checks completion, zero angles, cancellation before/after a reference,
  accepted Copy followed by cancellation, hidden polysurface options,
  RememberCopyOptions, Undo/Redo, explicit reselection and a new document.
  Interactive Rust replay compares all defaults, object order, sampled geometry
  and selection with the terminal snapshots.
- [32 interactive previews](../../tools/rhino_oracle/fixtures/twist_preview.json),
  with raw PNGs, pending model snapshots, completed geometry and public SDK
  control cages. These cover the three display modes, PreserveStructure, rigid
  groups, repeated Copy, initial reference prompts, invalid axial picks,
  a perspective view, Infinite, and a 450° mouse path. Rust compares curve and
  surface controls and box wires to the public SDK previews at `1e-11` and rigid
  placements at `1e-7`. Pixel witnesses check Line, Surface and Box paths and source face styling;
  they do not establish pixel identity or arbitrary-shape preview parity.
- [One supplementary B-rep capture](../../tools/rhino_oracle/fixtures/twist_preview_edges.json)
  retains all 12 quick-preview edge definitions, compared at `1e-11` with
  independent edge order and direction.

Raw observations are beside the fixtures under `observations/`, including
terminal command events and snapshots. These witnesses establish the measured
behavior; they are not exhaustive Rhino parity. The
[provenance record](../twist-provenance.json) retains artifact hashes and
comparison bounds.

## Limits and verification

Curves and surfaces use the existing sampled adaptive morph fitters, capped at
512 curve controls, 256 controls per surface axis and one million surface
samples. Shared B-rep edges and fitted surfaces must validate together at the
effective command fitting tolerance. This is sampled validation, not a continuous
error proof. The kernel's tighter explicit fits, cardinal cleanup transitions,
singular rational geometry, collapsed mesh facets or exhausted fitting budgets
can return an error. Preserved trimmed faces still pass assembly validation at
the document tolerance. Preview cages approximate the deformation and
can differ from fitted or preserved accepted geometry. Failed cage preparation
retains the previous display. Rational and higher-degree patches use mapped
controls as a display approximation; their native preview policy and arbitrary
singular or complex trimmed previews are not covered by these captures.

```sh
cargo test -p viboceros-command twist --lib
cargo test -p viboceros-geometry twist --lib
cargo test -p viboceros --bin viboceros app::tests::twist
python3 -m unittest tools.rhino_oracle.test_twist
python3 -m unittest tools.rhino_oracle.test_twist_preview
cargo test -p viboceros --bin viboceros viewport::twist_preview
cargo test -p viboceros --bin viboceros twist_gpu_overlay -- --ignored --nocapture
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/twist_points.json \
  --observations tools/rhino_oracle/observations/twist_points.json \
  --absolute-epsilon 1e-11 --relative-epsilon 1e-12
```

Command fixtures are native capture recipes, replayed through Rust command
tests. Regenerate a recipe in an owned settings scheme:

```sh
tools/rhino_oracle/run_headless.sh exec python3 - <<'PY'
import json
from pathlib import Path
from tools.rhino_oracle.client import OracleClient, load_request
request = load_request('tools/rhino_oracle/fixtures/twist_command.json')
result = OracleClient(settings_scheme='VibocerosOracleTwist').run_rhino(request, 300)
Path('/tmp/twist-command.json').write_text(json.dumps(result))
PY
```

Capture the pending previews in a separate Xvfb:

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.twist_preview_input \
  tools/rhino_oracle/fixtures/twist_preview.json \
  --output /tmp/twist-preview.json --scheme VibocerosOracleTwistPreview --timeout 420
```

The point timing capture measures batches of public SDK calls through Python
and Wine/FEX; it includes that API overhead and cannot establish a native Rhino
kernel speed comparison. The [release point report](../twist-point-comparison.json)
matches all 14 maps with maximum coordinate error `5.33e-15`; Rust batches
measured approximately 72–172 ns per point, including result collection.
See [timing interpretation](../oracle.md#timing-interpretation).

References: [Rhino Twist command](https://docs.mcneel.com/rhino/8/help/en-us/commands/twist.htm),
[public TwistSpaceMorph API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/T_Rhino_Geometry_Morphs_TwistSpaceMorph.htm),
and [licensed cardinal-angle adaptation](../../third_party/opennurbs_rust/README.md).
