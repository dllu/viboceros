# Native apex extrusion

Linux builds with `--features native-smlib` use usd-brep for ExtrudeCrvToPoint's
sheet and capped-solid geometry. Parsing and transactional document policy now
live in the separate `extrude_apex` command module. The direct Rust apex surface
and cap constructors are excluded from this command in native builds.

The bridge copies the native profile, uses `SetControlPoint` to collapse that
copy to the apex while retaining each corresponding weight, and calls the
existing `CreateRuledSurface` constructor. The matching weights make the
surface an affine profile-to-apex interpolation even for rational profiles.
Native reparameterization retains linear U, profile V, and the distance from
the profile start to the apex as the U domain. Surface transfer reuses the
existing owned NURBS snapshot and decoder.

Capped closed planar profiles use `CreateEndCapsFromSurface` on an owned B-rep
configured with document tolerance. The adapter rejects an apex in the profile
plane, checks native stitching and manifold status, and validates the exported
B-rep. Native singular topology and the existing pole connector transfer retain
the collapsed apex. Input native curves remain owned and usable.

Profile preparation is shared with the straight extrusion and revolution
adapters, including [native active-interval clamping](smlib-curve-import.md) and
uniform negative-gauge handling.
Existing affine transforms move construction to the apex origin and restore
world placement afterward. This is not qualification of all scale/aspect-ratio
extremes. Mixed-sign weights and discontinuous interior knots remain restricted.

All results are staged before document changes. Current-layer output attributes,
source deletion/retention, groups, selection and Undo/Redo retain the existing
command flow. A later invalid apex path preserves the entire batch and redo.
Open/nonplanar profiles remain sheets when Solid is requested.

The [live sheet fixture](../tools/rhino_oracle/fixtures/curve_native_apex.json)
and [raw Rhino observations](../tools/rhino_oracle/observations/curve_native_apex.json)
cover a line, a kinked polyline and a negative-height apex. Rhino's
SplitAtTangents and CreaseSplitting are disabled for this tensor comparison.
All three complete NURBS definition records match at `1e-10` absolute / `1e-12`
relative epsilon, without parameter or control-net normalization. See the
[comparison](smlib-apex-comparison.json) and
[official command options](https://docs.mcneel.com/rhino/8/help/en-us/commands/extrudecrvtopoint.htm).
Separate tangent-splitting controls for this app command remain further work.

Independent regressions check rational interpolation grids, actual native-curve
preservation, positive/negative oblique cone volume, invalid/coplanar apex
rejection and command history/metadata. These fixtures do not establish arbitrary
self-intersection, singular input, scale or Rhino performance parity. The native
cap helper's limitations are tracked in the [separate ledger](usd-brep-limitations.md).

Release validation passed 1,309 native command tests and 55 native bridge tests
(three tests ignored across those suites), two default-backend apex regressions,
and 1,095 Python oracle tests. Native clippy with warnings denied and workspace
formatting checks passed.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test apex
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib extrude_curve_to_point
```
