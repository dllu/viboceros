# Surface matching tolerance investigation

The [native catalog](surface-match-tolerance-investigation.json) retains 78
Rhino 8.32 observations from 14 completed private-display launches: 30 command
rows and 48 public SDK rows. Recipes, frozen producer sources, launch receipts
and logs are checked by:

```sh
python3 -m unittest tools.rhino_oracle.test_surface_match_tolerances
```

These are discovery results. Custom matching tolerances remain unavailable in
Viboceros, and the additional refinement cases have no qualified Rust replay.
One corrected enum-menu row has its own
[completed qualification](match-surface-menu-validation.json).

Observed command behavior:

- A fresh private scheme displays position `0.001`, tangency `1` degree and
  curvature `0.05` percent. Displayed model defaults lag document changes;
  explicit tolerance edits survive cancellation. Comparisons with SDK outputs
  suggest an unset fitting tolerance follows the current document.
- New scalar entries require position at least `1e-5`, tangency at least `0.1`
  degree and curvature at least the decimal `ON_SQRT_EPSILON`
  (`1.490116119385e-8`). Empty Enter retains smaller inherited defaults.
- Enter leaves the tolerance menu before accepting the match. Escape from a
  scalar getter cancels the whole command. Tolerance assignments outside the
  nested menu are unknown options in these captures.

Refinement density depends on tolerances and the initial source/target gap.
Position sweeps at a loose angular tolerance returned four, five or thirteen
transverse controls. Already coincident inputs retained four. In four captured
loose-angle cases, changing only curvature tolerance from `1e-5` to `100`
percent left every output control unchanged. This does not establish general
curvature behavior or certify continuity.

Public curve closest-point, local closest-point, surface closest-point in both
directions and curve-deviation queries returned near-machine-precision boundary
agreement. They have not explained the extra refinement. Three earlier local
query audit rows overwrote their recorded local fields; those diagnostic rows
are retained, with corrected queries in a later independent capture.
