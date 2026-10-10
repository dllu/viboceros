# Divide geometry and output qualification

The command implementation now lives in a dedicated Divide module. It adds
EqualChordLength, Split, DeleteRemainder and GroupOutput, with staged geometry,
source-domain trimming, current-layer point attributes and one Undo step.
Native captures disproved the preceding source-layer point policy; the source
curve keeps its attributes while generated points use the current layer.

The [30 recipes](../tools/rhino_oracle/fixtures/divide_command.json) drive actual
licensed Rhino Divide commands under private Xvfb. They cover count, length and
chord modes on a line, circle and polyline; point endpoints; curve splitting;
remainder deletion; and two-source output groups. [Observations](../tools/rhino_oracle/observations/divide_command.json)
retain point/curve geometry, seventeen curve samples, parameter domains,
closedness, source retention, layer/name/color-source attributes and group
memberships in object creation order. No coordinate sorting, curve fitting,
domain exclusions or geometric normalization is applied.

Closed curves require a seam stage before numeric division input. Accepting
that stage with Enter preserves the source seam. An early harness omitted this
stage and waited at the native prompt; adding Natural followed by Enter also
consumed the division default too soon. The qualified harness uses one Enter
for the seam and then supplies all division options and the numeric value.
Command workflows run at Rhino idle, after RunPythonScript returns.

The chord kernel constructs the sphere equation from each rational Bezier span
using exact dyadic coordinates, weights and radius. Bernstein sign variation
and exact subdivision isolate the first forward root, including even-multiplicity
contacts at a turnaround. Dedicated regressions cover corner crossing, a tangent
contact and invalid distances. Analytic line/circle/arc paths avoid repeated
polynomial work for those primitives. Full Rhino performance parity remains
unproven.

```sh
WINEDLLOVERRIDES='sspicli,secur32,schannel=b' \
  tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/divide_command.json \
  --scheme VibocerosOracleDivideQualified20261009 --timeout 240 \
  --output /tmp/viboceros-divide-qualified-native.json
cargo test -p viboceros-oracle --release --test divide
```

[Provenance](divide-command-provenance.json) records exact sources and validation.
Broader rational curves, extreme coordinates/domains, default preference memory,
interactive seam/direction changes and SubCrv selection remain
qualification/implementation work. [Bare prompting and station previews](divide-input.md)
were added in a later checkpoint; these command recipes do not qualify their
native interaction fidelity. These recipes do not
establish full Divide or Rhino compatibility.

The replay passes 27 of 30 recipes at 1e-9 absolute plus 1e-12 relative epsilon.
The three circular-NURBS length recipes remain numerical diagnostics at that
threshold; their maximum field difference is 1.1752123363351075e-7. All thirty
pass at 5e-7 absolute plus 1e-12 relative epsilon, with no field exclusions. The
integration test preserves the tighter epsilon for the other 27 cases and uses
5e-7 for these three; it does not establish tighter numerical parity. Both
reports remain in [comparison](divide-command-comparison.json).

Native source inspection confirms Circle recipes become ArcCurve objects.
The local oracle now preserves their analytic counterpart, correcting an
earlier NURBS promotion. Explicit NURBS-circle captures instead preserve rational
split parameterization; the earlier whole-circle analytic carrier has been
removed. Length-mode MarkEnds does not add an incomplete final endpoint in the
native captures. The [curved checkpoint](divide-curved.md) records the source
inspection and parameterization correction, while the preceding comparison
remains historical evidence from its recorded implementation.

The later [curved qualification](divide-curved.md) adds arcs, ellipses,
rational/nonrational NURBS and mixed polycurves, plus extreme-weight and
non-dyadic-contact diagnostics. It fixes the chord root-depth cutoff and removes
the chord kernel's dependence on arc-length integration. Its two extreme Split
records remain unresolved; the raw outputs and comparisons are retained.
