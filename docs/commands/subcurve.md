# SubCrv

[Curve commands](curves.md) · [Numeric UV inputs](../subcurve-length-confirmation.md)

Start `SubCrv`, select a curve if none is preselected, and pick its start.
Then pick an end location or enter a length followed by a direction-confirmation
location. A new number replaces the pending length; units and calculator
expressions use the shared point-input parser. `Copy=Yes|No` can be supplied at
start or changed during the getter. Zero, empty Enter and Escape cancel without
geometry or history changes and release source selection.
[Copy, Mode and FromMidpoint](../subcurve-option-memory.md) use independent session
memory: accepted changes survive cancellation and document history.

Open point intervals retain the source's orientation, regardless of pick order.
Closed point intervals follow the source through its seam. Numeric lengths use
their magnitude; closed confirmation chooses the nearer prospective endpoint.
Open numeric
intervals clamp at the chosen endpoint; a number exceeding the whole curve is
rejected for retry. A full closed traversal is retained by standalone SubCrv,
while the inline UV getter omits that temporary input.

Copy keeps the original geometry and puts the new curve in the original layer
and groups, preserving its name and user text. The result is selected and the
original is unselected. Replacement keeps object identity and attributes and
releases selection. Undo/Redo follow the captured selection states.

For scripts, select one curve and use:

```text
SubCrv start_point end_point Copy=Yes
SubCrv Numeric=anchor,length,confirmation_parameter Copy=No
SubCrv Parameter=start,end Copy=Yes
```

The explicit `Parameter` extension remains a directed kernel interval: decreasing
open parameters reverse the result. Numeric parameters belong to the original
source domain. The shared command policy is independent of the kernel's signed,
unclamped arc-length query.

The [17 closed recipes](../../tools/rhino_oracle/fixtures/standalone_subcurve.json)
ran on private Xvfb with scheme `VibocerosOracleStandaloneSubcurveFinal20261007`.
[Raw records](../../tools/rhino_oracle/observations/standalone_subcurve.json)
retain original/output definitions, 33 stations per curve, layers, names, user
text, groups, selection, command events, and Undo/Redo. Fifteen commands succeed;
zero and missing confirmation remain native cancellations. Command and app
replays compare loci and directed endpoints at `1e-6`, metadata, source retention
and history. See [provenance](../standalone-subcurve-provenance.json).

[`Mode=MarkEnds`](../subcurve-mark-ends.md) creates default endpoint points while
retaining the source, with current-layer output and unselected history states.
[`FromMidpoint=Yes`](../subcurve-midpoint.md) builds an interval around the first
pick, with immediate half-length entry or symmetric endpoint picking.
[Direction locking](../subcurve-direction.md) captures the hovered side for
numeric entry and is shared with inline UV input. Some closed lengths retain a
[point confirmation](../subcurve-direction-confirmation.md). B-rep edge
references and restart preference persistence remain outstanding. The recorded
cases do not establish arbitrary-curve or preview parity.

```sh
cargo test -p viboceros-command --release standalone
cargo test --release --bin viboceros standalone_subcurve
```
