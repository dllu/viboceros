# SubCrv direction locking

[SubCrv](commands/subcurve.md) · [Inline UV input](uv-subcurve-input.md) · [Numeric input](subcurve-length-confirmation.md)

After picking the start, move the cursor along the source curve and enter `D`
or `Direction=Locked`. The command captures that side of the anchor; later
cursor movement does not change it. `Direction=Free` restores ordinary input.
Direction belongs to the current getter and is reset for a new command. It does
not change Copy, Mode or FromMidpoint session preferences or document history.

A length entered while locked on an open source completes the piece immediately,
using the number's magnitude. Open sides clamp at their natural endpoints. The
output retains source orientation, including backward extraction. Shorten, MarkEnds and inline
CreateUVCrv/ApplyCrv input share this direction policy. A typed endpoint on the
opposite side of a locked open source produces no piece; closed typed endpoints
use the ordinary source-forward interval.

Closed numeric input can complete immediately or retain its length for a
confirmation point. The source and anchor stay intact; confirmation can choose
a new side. The generic `Select curve` prompt remains a point getter. See the
[closed confirmation follow-up](subcurve-direction-confirmation.md), which
corrects the earlier interpretation of cancelled numeric inputs.

The scripting adapter accepts an explicit direction without requiring a cursor:

```text
SubCrv Numeric=anchor_parameter,length,unused_confirmation Locked=Forward Copy=Yes
SubCrv Numeric=anchor_parameter,length,unused_confirmation Locked=Backward Mode=MarkEnds
```

Viewport output carries the resolved drafting hover location separately from an
accepted pick. The application projects it onto the current source, captures its
direction, and calls the shared command policy for extraction. Source geometry
is never modified by hovering or changing Direction. Inline input remains a
pending range until its parent UV command completes.

The [18 bounded recipes](../tools/rhino_oracle/fixtures/subcurve_direction.json)
ran through the public Rhino getters on private Xvfb under the settings scheme
`VibocerosOracleDirectionVerified20261007`. Ten commands succeeded; four open
point inputs ended with Failure and four closed numeric inputs with Cancel.
[Raw records](../tools/rhino_oracle/observations/subcurve_direction.json)
retain motion acknowledgements, locked/unlocked prompts, command-end events,
original and output definitions, curve stations, markers, selection and
Undo/Redo. The input driver acknowledges real motion in the owned window. The
worker then sends the fixed tokens through public `RhinoApp.SendKeystrokes`, waits for
the corresponding prompts and records every token without repeating endpoints.
See [provenance](subcurve-direction-provenance.json).

Application replay compares all recorded curve stations and directed endpoints
or marker coordinates at `1e-6`, plus source retention, selection and history.
This capture covers lines, a closed polyline and a rational circle; it does not
establish arbitrary-curve or midpoint-and-lock combinations, live
preview, edge input, restart preference persistence or reactive History.

```sh
cargo test --release --bin viboceros subcurve_direction
cargo test -p viboceros-command --release locked_subcurve
python3 -m unittest tools.rhino_oracle.test_subcurve_direction
```
