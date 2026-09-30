# OpenNURBS Rust adaptations

`chord_adjust.rs` adapts the point-list/NURBS chord-adjustment policy from
`opennurbs_brep.cpp` (`AdjustPointListAlongChord`, `AdjustNurbsCurve`) in the
pinned OpenNURBS submodule at `23fc677ba06e49212296ca75fab7fb6c2851b4ce`.
The original source copyright notice is retained; see [LICENSE](LICENSE) and
[McNeel's developer-tools license](https://developer.rhino3d.com/license/).

This modified Rust port uses exact rational chord projection and displacement
with one final rounding per coordinate. It keeps the short-chord fallback;
closed equal-endpoint curves translate without a minimum displacement cutoff.
The geometry kernel retains weights/knots and certifies resulting uncertainty.
No proprietary Rhino source was used. This is not an upstream McNeel component.

`viewport_clipping.rs` adapts the constrained `ON_Viewport::SetFrustumNearFar`
overload from `opennurbs_viewport.cpp` at the same pinned revision. It retains
the original copyright/disclaimer above and uses the [developer-tools MIT
license](LICENSE). The modified Rust routine returns near/far distances and a
parallel camera dolly rather than mutating an OpenNURBS viewport. It rejects
nonfinite inputs and uses the measured Rhino `ViewportInfo` default of 0.0005
for an invalid ratio constraint. Viboceros's document clipping padding and
frustum intersection implementation remain in `src/viewport/clipping.rs`;
those are independently calibrated/implemented from public outputs and math.
