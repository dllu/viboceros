# Inline SubCrv length confirmation

[Temporary UV inputs](uv-subcurve-input.md) · [Signed-length kernel](signed-length-subcurves.md)

At either UV command's source-selection prompt, type `SubCrv`, choose one curve,
pick its start, and enter a length. Then pick or type a confirmation location.
The location chooses the side of the start; it does not set the resulting length.
A new number replaces the pending length. Explicit units and the existing point
calculator are supported. No temporary document object or Undo entry is created
until the parent command finishes.

Numeric inputs use their magnitude. Results retain the source's orientation,
including when the chosen interval extends backward from the start. Closed
curves choose the nearer of the two prospective numeric endpoints. The
[closed direction follow-up](subcurve-direction-confirmation.md) covers nearby
branches and pending locked input. The length must
not exceed the whole source curve; an open interval clamps at its chosen endpoint.
A coincident confirmation chooses backward. A complete closed traversal produces
no temporary input. Zero and empty Enter abandon the current nested getter and
return to the parent selection without discarding already accepted inputs.
Escape cancels the parent and restores its initial whole-object selection.

Curve IDs identify sources, but a numeric length requires a location to confirm
the direction. This explains the earlier captures that entered a length and then
ended the getter or supplied another object ID: they did not supply this pick.
The mathematical `SubCrvLength` script option remains independently signed and
does not clamp unavailable lengths.

The [29 closed recipes](../tools/rhino_oracle/fixtures/subcurve_numeric_followup.json)
ran on private Xvfb under `VibocerosOracleSubcurveConfirmCommands20261007`.
[Raw records](../tools/rhino_oracle/observations/subcurve_numeric_followup.json)
retain original definitions, 50 output curves with 1,650 stations, getter history
and command-end events. Twenty-eight commands succeed; the standalone object-ID
reference attempt remains a native cancellation. Actual mouse confirmation and
typed-coordinate cases cover both directions, negative/replacement numbers,
curved inputs, closed seams, nonuniform segment lengths, clamping and no-input
transitions. Application tests replay all 27 UV-command cases through the actual
controller, checking loci and directed endpoints at `1e-6`, source purity and
Undo/Redo. [Standalone SubCrv](commands/subcurve.md) now has its own source and
numeric controller. [Direction locking](subcurve-direction.md) is shared by
standalone and inline input; B-rep edge input remains outstanding.
See [provenance](subcurve-length-confirmation-provenance.json).

The geometry kernel additionally returns the endpoint in the original native
domain alongside the arc-length piece. Seam crossings use parameter wrapping,
not closest-point recovery; a self-crossing regression preserves the correct
branch when two native parameters have the same spatial point.

```sh
cargo test --release --bin viboceros app::tests::uv_subcurve_input
cargo test -p viboceros-drafting --release length_quantity_tests
cargo test -p viboceros-geometry --release original_endpoint_parameters
```
