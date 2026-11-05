# Curve and surface-edge chain comparisons

[Command behavior](../commands/curve-chains.md) · [Oracle overview](../oracle.md)

The `chain_selection_*` fixtures construct owned lines, circular arcs, circles
and quad B-rep surfaces. They record the actual Rhino 8 command through public
selection, highlight, EndCommand and command-history APIs. Each capture uses
private Xvfb, an isolated runtime directory and a private settings scheme.
Compressed observations and content-addressed producer snapshots are immutable.

The 146 standalone workflows check entry, preselection, cancellation, repeated picks, direction,
continuity, gaps, angle boundaries, closed paths, mixed curve/edge preference,
branch proposals, explicit alternatives, per-segment Undo, Next and All.
Every source CRC remains unchanged. A seeded Redo/Undo cycle verifies selection
survives without consuming model history. Negative branch controls retain
Rhino's unknown `Next` response and its global `All`/`Undo` behavior.

Native curvature tests bracket an absolute curvature-vector difference of
`sqrt(f64::EPSILON)` in inverse model units. Equal-radius tangential arcs pass;
different radii can fail even within OpenNURBS' public relative G2 test.
Scaled arcs distinguish absolute from relative thresholds. Tangent tests retain
Rhino's rounded-dot angle behavior at the measured one-degree boundary.
Zero numeric options restore document tolerances; positive values below the
document tolerance are accepted. Controls bracket the numeric lower bound at
OpenNURBS' public `ON_ZERO_TOLERANCE` (`2^-32`, applied to distance or radians).

Rust builds the sources independently from the request, without captured output
or topology as input. Protocol and app replays compare before, every input step,
after, redo, undo, success and source preservation. Native quad faces mark trims
and later edges; comparisons project their one-to-one trim/edge correspondence
to a single model-space edge selection. Original native flags remain retained.
This projection is specific to these simple source surfaces.

The Python API exposes `OracleClient().run_viboceros(request)`; requests are in
`tools/rhino_oracle/fixtures/chain_selection_*.json`. Run the discovery generator
or another matching generator through the private-display wrapper:

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_chain_selection_discovery
```

Existing successful families cannot be overwritten.

In-memory egui pointer events cover curves, surface edges and branch choices
in all three display modes without opening windows. A 20,000-segment chain
with unrelated dense endpoints checks traversal and redo preservation. Endpoint
queries reuse Join's balanced spatial index; source geometry is prepared once.
These controls establish selection behavior for the measured sources, not
arbitrary trimmed-B-rep topology, other nested getters or performance parity with Rhino.

Seventeen `length_chain_*` workflows additionally qualify SelChain inside Length:
parent/seed/connected/branch/unit prompts, re-entry, cancellation, surface-edge
measurement, mixed inputs and redo/undo. Native middle-seed Undo controls establish
forward-before-backward growth for Both direction; each Undo removes one segment.
Rust constructs the sources independently and replays shared selection and Length
commands. App tests replay every input and use real egui events for ordinary
curve/edge picks in all display modes.

Comparisons retain raw prompt text and component flags, compare getter phases
and settled selection, and normalize lengths to millimeters within native printed
precision. Older EndCommand notifications precede selection cleanup; their terminal
trace uses the settled after-state. Follow-up captures explicitly sample after
RunScript returns and confirm the cleared selection. Source CRCs remain unchanged.
Rhino accepts `Units`, then singular `Centimeter`; a retained negative control
rejects typed `Units=Centimeters`. Viboceros accepts that existing shorthand and
reports the equivalent physical length. These tests do not establish remembered
display-unit defaults or nested SubCrv behavior.
