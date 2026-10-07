# Closed SubCrv numeric confirmation

[SubCrv](commands/subcurve.md) · [Direction locking](subcurve-direction.md) · [Inline numeric input](subcurve-length-confirmation.md)

A closed curve can accept a locked numeric length immediately, or keep the
length pending and display `Select curve`. That second prompt remains a point
getter: the source, anchor and length survive, no curve is created, and no Undo
entry is added. A confirmation point completes the length. Enter cancels it.
This also applies to the nested CreateUVCrv source getter, whose parent owns the
final transaction.

The earlier direction capture mistook that prompt for a new source selection
and treated valid intervals as unavailable. The controller now retains the
original input and returns to free numeric confirmation. The explicit scripting
adapter's `Locked=Forward|Backward` always extracts a valid directed length;
interactive confirmation is separate from curve extraction.

The captured lock follows the cursor's side in the original parameter chart.
The observed closed getter asks for confirmation when the resulting interval
crosses the original domain midpoint. The skewed polyline separates this
parameter midpoint from half the perimeter. This transition is a policy
inferred from the recorded charts, rather than a general native API guarantee.

Closed free confirmation chooses between the two prospective numeric endpoints
by their distance to the picked curve location. On nearby branches, this can
choose a different side from the shorter arc toward that location. The skewed
source demonstrates that distinction. A coincident confirmation retains the
backward default. Both candidates come from parameter-preserving signed length
queries; the selected interval is reused rather than solved a third time.

The [32 bounded recipes](../tools/rhino_oracle/fixtures/subcurve_direction_grid.json)
ran on private Xvfb with scheme
`VibocerosOracleClosedDirectionComplete20261007`. They cover rectangles,
rational circles, a skewed polyline, two anchor positions, two lengths, both
cursor sides, same/opposite confirmations and two nested UV workflows.
[Raw records](../tools/rhino_oracle/observations/subcurve_direction_grid.json)
retain every input, mouse acknowledgements, public point/object getter flags,
intermediate document snapshots, complete source/output definitions, selection,
command results and independent Undo/Redo states. Six cancelled recipes keep
an unconfirmed number; 26 commands succeed. Sixteen recipes retain a pending
point-getter snapshot.

App replay checks all inputs, the intermediate source/start/length state,
unchanged sources and absent history, then 924 output curve stations and directed
endpoints at `1e-6`, selection and Undo/Redo. Command regressions independently
exercise both explicit closed directions and the skewed endpoint choice.
See [provenance](subcurve-direction-confirmation-provenance.json).

Arbitrary closed charts, exact endpoint ties, complete locked traversals,
midpoint-plus-lock combinations, native preview-style parity and B-rep edge input need wider
native coverage. The station comparisons do not certify a continuous locus or
establish general performance parity.

```sh
cargo test --release --bin viboceros subcurve
cargo test -p viboceros-command --release direction
python3 -m unittest tools.rhino_oracle.test_subcurve_direction_grid
```

[Cached viewport previews](subcurve-preview.md) display the pending curve or
endpoint markers without document edits.
