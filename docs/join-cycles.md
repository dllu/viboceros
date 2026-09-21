# Join cycles and command boundaries

[Join and JoinCopy](commands/join.md) · [Kernel policies](curve-editing.md)

An individually picked chain ends immediately when it closes. No final Enter is
needed. The app checks completion without changing geometry or history, then
executes one atomic command. Later clicks belong to the next interaction.
Early `JoinCopy` completion retains only participating originals selected; its
new closed output is unselected. Skipped sources are not copied or retained in
that selection. Command-first mesh copies also retain their source selection.

## Parameters and representation

- Wholly linear batches use chord lengths and cut a loop at its final
  source-ordered connection. Direction ties favor the last source.
- Mixed batches keep native interval widths, retain seed direction on a tie,
  and restore closed seams to the earliest source. An unrelated open nonlinear
  curve can select this assembly path without changing a linear output's type.
  Existing closed inputs are filtered before command assembly.
- Individual picks keep the seed's direction and original parameter interval.
  A closing input normally prepends, producing a negative domain start when
  appropriate. If it can merge with a linear tail behind a nonlinear head, it
  appends instead. Closed `JoinCopy` moves the seam back to the seed start.
- Adjacent exact line/polyline leaves coalesce into a polyline without fitting
  or changing parent parameter speeds. Batch and open individual results
  synchronize leaf domains to parent intervals. Early closed individual results
  retain independent local domains. Both are recorded and compared explicitly.
  Mixed linear-run consolidation and rational seed exceptions are detailed in
  [curve encodings](join-encodings.md).

Endpoint matching remains separate from representation/parameter assembly in
`curve_join/assembly`. Public OpenNURBS joining and polycurve-domain routines
were inspected as compatibility references; the Rust implementation uses its
own bounded endpoint graph and checked exact-curve constructors.

## Evidence and measurement correction

The [284-case fixture](../tools/rhino_oracle/fixtures/join_cycles.json),
[raw observations](../tools/rhino_oracle/observations/join_cycles.json), and
[comparison report](join-cycles-comparison.json) include all six triangle source
orders, several direction masks, unequal lengths, unrelated nonlinear/closed
inputs, skipped picks, rectangles, two-source loops, arc/line loops, semicircles,
and open controls, for both commands and both selection modes. Every recorded
field is compared in creation order: native types, local/parent domains, samples,
source retention, attributes, groups, and selection. Numeric tolerances are
absolute `1e-10`, relative `1e-12`; no order or parameter normalization is used.

The old probe recorded the whole driving macro's result and final document.
Public [EndCommand](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/E_Rhino_Commands_Command_EndCommand.htm)
transcripts show why that was incorrect: after early closure,
later SelID tokens select unrelated objects outside Join. Even without trailing
selectors, macro cleanup can change selection, and RunScript success can hide a
named command returning `Nothing`. The probe now captures exactly one named
EndCommand result and snapshot. It detaches in a finally block and fails on
missing/duplicate completions or swallowed callback exceptions.

Eight [trace requests](../tools/rhino_oracle/fixtures/join_command_events.json)
retain [raw event transcripts](../tools/rhino_oracle/observations/join_command_events.json).
The earlier four [macro-tail observations](../tools/rhino_oracle/observations/join_closed_chain_diagnostic.json)
are preserved as historical evidence, not current command-boundary expectations.
Their requests are included in the new passing cycle suite. The existing 140
workflow and 181 mesh observations were re-recorded at the corrected boundary;
24 workflow and four mesh cases changed result/selection, not geometry. Earlier versions remain
available in Git history.

Independent tests cover seed-interval preservation, parent derivatives on both
sides of merged vertices, read-only completion checks, exact source retention,
undo/redo, and the first click after automatic completion. These bounded cases
do not establish arbitrary curve-network compatibility or performance parity.
Batch/window picks within a command, prompt Undo, B-reps, mixed families, and
subobjects remain outside this implementation.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/join_cycles.json --timeout 540 --relative-epsilon 1e-12
cargo test --release -p viboceros-oracle join_command::tests
```
