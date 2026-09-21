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
