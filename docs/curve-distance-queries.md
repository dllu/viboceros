# Curve distance queries

The native usd-brep adapter now exposes `curve_distance_extrema` for two curves
and explicit normalized source intervals. The registered
[`CrvDeviation` command](commands/crv-deviation.md) now uses this backend for
curved inputs; its overlap policy and reported minima still need compatibility
work. The backend and its earlier diagnostic capture remain separate from
command behavior.

Enable `native-smlib` in the application/oracle or `native` in
`viboceros-smlib`. The producer accepts the existing `CurveRef` families through
the shared canonical NURBS importer and normalized working frame. Every returned
witness contains normalized NURBS fractions, original source parameters, two
source points and their actual distance. The shared `parameter_from_nurbs` map
recovers native parameters for every curve family, preserving the requested
visit through circular seams and composite arc branches without an additional
closest-point query. Original source images validate the mapped parameters. The maximum
witness also passes an independent closest-point check on the requested source
portions in at least one direction; the minimum must pass in both directions.
Numerical roundoff within the requested tolerance that reverses minimum/
maximum order is handled by selecting the attained minimum witness for both.

The implementation reuses SMLib's global curve minimum, global point projection,
curve maximum and local curve solvers. Upstream maximum pairs are proposals:
the local maximum solve can leave the nearest-point branch. Global reprojection
prevents a farthest pair from being reported as a nearest-curve deviation. Native
knot, span-midpoint and grid samples supply additional candidates, and sampled local peaks use
the existing curve solver before reprojection. No additional spline or matrix
solver is introduced and the pinned upstream source is unchanged.

This is a bounded numerical search. It does not certify global extrema or a
continuous Hausdorff bound. Source intervals are explicit because selecting
corresponding/overlapping portions is separate from measuring their distances.
The diagnostic `projected_curve_intervals` helper projects endpoints of open
curves and retains full closed domains; it does not implement Rhino's complete
overlap policy. The command separately preserves corresponding transverse
natural ends; the diagnostic helper's original policy is unchanged. Empty
projected intervals return no report. Canonical working
frame limitations remain, including degree/weight and discontinuity restrictions.
Queries allow at most 4096 controls per source. Invalid, reversed, nonfinite or
out-of-domain interval values reject; equal interval ends return no report.

The [18-case native capture](../tools/rhino_oracle/observations/curve_deviation_rhino_20261016.json)
retains both public `Curve.GetDistancesBetweenCurves` witnesses and actual
`CrvDeviation` command histories. Inputs include parallel/partial/reversed lines,
non-overlapping and perpendicular lines, coincident curves, concentric and
translated circles, a circle/chord, planar/spatial cubics, polylines, remote
coordinates and postselection. The native command and SDK agree on 13 successful
reports and five no-overlap decisions. Sources remain intact and no model
geometry is created. The private command macro appends Enter and Cancel with
KeepMarks left at its default No.

For the 13 successful native SDK reports, all maximum scalar values and 11
minimum scalar values agree within `1e-6`. Two differences remain:

- The planar offset cubics produce a local minimum witness around `0.657680`,
  while Rhino reports `0.682468`.
- The spatial offset cubics produce a local minimum witness around `0.931594`,
  while Rhino reports `0.935192`.

Exact rational evaluation of the original polynomial Bézier inputs supplies
closer source pairs than the recorded native minima in both cases. This is
evidence that those native values differ from the whole-interval minimum;
it does not establish the producer's global completeness. The differences are
retained as failing comparisons.

The diagnostic endpoint policy also accepts crossing and separated circle
pairs that native CrvDeviation declines. Overall scalar/overlap parity remains
false. Witness locations and parameters can differ for tied extrema even when
the distances agree; no directed witness or complete interval parity is claimed.

The `curve_deviation` oracle operation retains the original two curve recipes
and `postselect` flag. Its Rhino side captures the actual command and SDK. Its
local side runs the backend only when `keep_marks` is omitted. The original
committed response records `command_registered=false` at its capture revision;
replaying now reports the command's current registration. An explicit
`keep_marks` boolean selects the new command workflow instead.
The backend diagnostic requires `native-smlib` locally and a private settings scheme with one
iteration on Rhino. The initial capture used command echo off and retained
empty histories; its raw SDK observations remain in a separate initial capture.
The final capture enables echo and requires a nonempty history.

```sh
cargo run --quiet -p viboceros-oracle --features native-smlib -- \
  tools/rhino_oracle/fixtures/curve_deviation.json /tmp/curve-distances.json
```

See [McNeel's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/crvdeviation.htm),
[SDK reference](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/getdistancesbetweencurves),
the [comparison](../tools/rhino_oracle/observations/curve_deviation_comparison_20261016.json)
and [validation record](curve-distance-validation.json). The address-sanitizer
probe instruments the current C ABI bridge and driver against existing native
archives; vendor/helper archives remain uninstrumented. It provides bridge-level
coverage, not full-kernel sanitizer or leak qualification. No benchmark or
performance qualification is claimed.
