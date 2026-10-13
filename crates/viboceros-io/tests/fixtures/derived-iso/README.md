# Stale derived isocurve flags

`stale_flags.3dm` is an original public-OpenNURBS fixture containing a unit box
and a right-triangle planar face. `generate.cpp` first constructs and validates
both B-reps, then deliberately changes one derived trim isocurve flag on each:
a boundary flag becomes `not_iso`, and a diagonal `not_iso` flag becomes South.
No control points, knots, vertices, or incidence records are changed.

The source archive intentionally fails native B-rep validity. Recomputing
`SetTrimIsoFlags()` on an import copy restores validity. The Rust integration test
checks both objects survive import, retain their unit volume / half-unit area,
and round-trip with unchanged decoded geometry.

```sh
cargo test -p viboceros-io --test derived_iso
```

To regenerate, compile `generate.cpp` as C++17 linked to `opennurbsStatic` and
its vendored zlib, freetype and UUID archives, as in the
[pushup audit](../pushup/README.md). Run the executable with the output 3DM path.
Initial SHA-256:
`f70f98d402f94ffac6f7f87ab1efca0271af65bb7d456294bd02e3588934979f`.
