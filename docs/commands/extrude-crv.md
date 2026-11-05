# ExtrudeCrv

Select curves, then enter a distance or two direction points:

```text
ExtrudeCrv 3 Solid=Yes BothSides=Yes DeleteInput=No
ExtrudeCrv 0,0,0 1,2,3 Output=Surface
UseExtrusions Polysurface
ExtrudeCrv 3 SplitAtTangents=Yes
```

A distance uses the common normal of parallel planar profiles, oriented with
its largest world-axis component positive. Other profiles and a single line
use the active CPlane Z; parallel offset lines use their containing plane.
Two points specify an arbitrary direction. Bare `ExtrudeCrv` starts the two-pick
viewport workflow. Options can change before either point without losing the
base point. See [numeric direction qualification](../smlib-extrusion-direction.md).

`BothSides=Yes` sweeps equally in both directions. `Solid=Yes` caps closed
planar profiles; open and nonplanar profiles remain open. Lines parallel to the
path are skipped. `DeleteInput=Yes` removes only profiles that produced geometry.
Results have fresh attributes on the current layer, remain unselected and
ungrouped, and share one undo step with source deletion.

[UseExtrusions](use-extrusions.md) selects typed extrusion storage for eligible
perpendicular planar sweeps. Oblique and nonplanar sweeps use explicit B-reps.
Profile parameters are retained, including polyline vertex intervals. Closed
profiles may reverse with the path. Typed results preserve the public support
plane's up direction when orienting the path; see [typed creators](../creator-extrusions.md).

`SplitAtTangents=Yes|No` is available for Surface output when UseExtrusions is
Polysurface. It defaults to Yes and is remembered after a successful command,
including across Undo/Redo. Cancelled or failed commands discard pending edits.
The remembered value also controls explicit B-rep fallback while UseExtrusions
is Extrusion, where the option is hidden. Eligible typed results retain their
canonical public OpenNURBS B-rep conversion.

Explicit sweeps split at G2 discontinuities using a one-degree angular test
when SplitAtTangents is enabled. Capped sweeps first split the profile at G1
joins using that angle, construct both caps and the walls, then split the
completed walls at G2. Global `CreaseSplitting` subsequently
splits G1 joins at 0.1 degrees, independently of SplitAtTangents. Exact shared
face partitioning preserves boundary incidence. Rational cap trims receive
native end-weight normalization after all subdivisions.

`Output=SubD` uses the [surface-to-SubD fitter](extrude-crv-subd.md). Switching to
SubD resets Solid and SplitAtTangents to No; switching back keeps those values.
BothSides and DeleteInput survive the switch. Other options reset each invocation.

The [validation record](../extrusion-splitting-validation.json) covers 163 native
workflows with both geometry builds, application replay and 3DM round trips.
Every used surface, spatial edge, UV trim, knot, weight, tolerance, face sense
and oriented incidence is compared at absolute epsilon `1e-12`, with before,
after, Undo and Redo state. Correspondence allows component renumbering, unused
storage padding knots and equivalent oriented cap-axis choices. Captures include
rational, periodic, reversed, nonplanar and mixed-continuity profiles, both path
directions, independent split flags, mode switches and cancellation.

Angular discontinuity admission uses sampling; a continuous maximum-angle
certificate and comparative performance remain unqualified. ToBoundary is
unsupported. Existing exact-partition and refinement budgets apply.

```sh
cargo test -p viboceros-oracle --test extrusion_splitting --test extrusion_splitting_archive
cargo test -p viboceros extrusion_splitting
python3 -m unittest tools.rhino_oracle.test_extrusion_splitting
```

Native captures run in private Xvfb sessions through
`tools/rhino_oracle/run_headless.sh`.
Reference: [Rhino ExtrudeCrv](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudecrv.htm).
