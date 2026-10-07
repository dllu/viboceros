# SubCrv MarkEnds

[SubCrv](commands/subcurve.md) · [Numeric confirmation](subcurve-length-confirmation.md)

`Mode=MarkEnds` places points at the accepted subcurve's endpoints while keeping
the original curve. Set it when starting `SubCrv`, or change `Mode=Shorten|MarkEnds`
during source/start/end input. Point picks and numeric length confirmation share
the existing standalone orientation, magnitude, seam and clamping policies.

Both points use default attributes on the current layer, have no source name or
user text, and belong to no groups. All objects are unselected after completion.
Copy does not change this mode's output. A full closed traversal produces two
coincident markers. The command owns one Undo entry; Undo/Redo keep selection
empty and restore the original marker IDs. Cancellation adds no history.

```text
SubCrv 1,1.5,0 3,4.5,0 Mode=MarkEnds
SubCrv Numeric=anchor,length,confirmation_parameter Mode=MarkEnds Copy=No
SubCrv Parameter=start,end Mode=MarkEnds
```

The explicit Parameter extension retains its directed mathematical interval.
All endpoint geometry is staged before insertion; invalid modes and failed
geometry queries roll back without markers or history changes.

The [15 closed recipes](../tools/rhino_oracle/fixtures/subcurve_mark_ends.json)
ran on private Xvfb under `VibocerosOracleMarkEnds20261007`.
[Raw records](../tools/rhino_oracle/observations/subcurve_mark_ends.json) retain
26 markers from 13 successful commands and two cancellations (zero and missing
confirmation), full original curve definitions, properties, groups, selection,
events, and Undo/Redo. Command/app replays compare every point within `1e-6`,
source purity, default attributes and history. Numeric curved inputs retain the
existing integration and native fitting limits. [FromMidpoint](subcurve-midpoint.md)
also supports MarkEnds. Direction locking,
B-rep edge references, native mode memory and reactive Rhino History remain
outstanding. See [provenance](subcurve-mark-ends-provenance.json).

```sh
cargo test -p viboceros-command --release mark_ends
cargo test --release --bin viboceros subcurve_mark_ends
```
