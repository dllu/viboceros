# Curve deviation overlap development

The command now preserves transverse natural endpoints of open curved profiles.
It first computes the existing closest-projection windows, then extends a
boundary when the corresponding natural endpoint displacement is perpendicular
to the aligned endpoint chords within absolute model tolerance. This handles
translated profiles and retains partial projected boundaries elsewhere.
Closed inputs and ambiguous chord frames keep their original windows. This is
an independent geometric policy; complete native overlap parity remains open.

Both command endpoint marks and native distance witnesses now use
`CurveRef::parameter_from_nurbs`. The shared representation map retains the
original parameter visit at circular seams and through composite arc leaves.
Previously a closest-point query could choose a different visit to the same
physical point. A native regression query on a circle's final quarter now
verifies that its unique maximum witness retains the final seam parameter.
Original source images still validate every native mapped distance witness.

Three owned headless captures add 52 cases:

| Capture | Cases | Purpose |
| --- | ---: | --- |
| [Profiles and asymmetric lines](../tools/rhino_oracle/fixtures/curve_deviation_overlap.json) | 25 | Full/partial/reversed cubics, linear NURBS, skew lines, folded polylines, arcs |
| [Endpoint offsets](../tools/rhino_oracle/fixtures/curve_deviation_endpoint_cones.json) | 17 | Parallel line endpoints at different longitudinal and transverse offsets |
| [Endpoint transition](../tools/rhino_oracle/fixtures/curve_deviation_endpoint_boundary.json) | 10 | Bracket a native interval transition with simple parallel segments |

These captures also disprove a universal chord-bisector interpretation of native
line overlap. For example, changing one retained parallel-line offset from
`2.2` to `2.23` changes native markers from natural endpoints to projected
interior endpoints. The transverse gap remains `2`. The diagnostic transition
recipes were chosen adaptively to test a distance-ratio hypothesis; no native
returned geometry or fitted threshold is used in the implementation. The
current line algorithm is retained as an explicit approximation, and the
console and window state that limitation for all input families.

The main corpus retains the
[initial local response](../tools/rhino_oracle/observations/curve_deviation_overlap_initial_viboceros_20261016.json),
[updated local response](../tools/rhino_oracle/observations/curve_deviation_overlap_viboceros_20261016.json),
and [native capture](../tools/rhino_oracle/observations/curve_deviation_overlap_rhino_20261016.json).
The [before](../tools/rhino_oracle/observations/curve_deviation_overlap_initial_comparison_20261016.json)
and [after](../tools/rhino_oracle/observations/curve_deviation_overlap_comparison_20261016.json)
comparisons preserve all failures. White interval-end marker sets improve from
9 to 16 matching cases out of 25, including one joint no-report case. Among
the 21 cases where both engines report, endpoint sets improve from 8 to 15.
This separate comparison does not accept
different minimum/maximum witness positions merely because their distances
agree. Asymmetric line boundaries, folded inputs, general open/closed
correspondence, and some reported extrema still differ from Rhino.

```sh
cargo run --quiet -p viboceros-oracle --features native-smlib -- \
  tools/rhino_oracle/fixtures/curve_deviation_overlap.json /tmp/crv-overlap.json
python3 -m tools.rhino_oracle.curve_deviation_comparison \
  tools/rhino_oracle/fixtures/curve_deviation_overlap.json /tmp/crv-overlap.json \
  tools/rhino_oracle/observations/curve_deviation_overlap_rhino_20261016.json \
  /tmp/crv-overlap-comparison.json --interval-ends
```

The original comparison mode remains the default so historical records can be
reproduced. `--interval-ends` adds a separate occurrence-preserving comparison
of white endpoint points and connecting lines. No global-extremum certificate,
complete command parity, benchmark, or additional sanitizer qualification is
claimed. See the [validation record](curve-deviation-overlap-validation.json).
