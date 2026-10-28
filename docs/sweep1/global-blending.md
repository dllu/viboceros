# Global Sweep1 blending

[Command guide](../sweep1.md) · [Validation](validation.md) · [Recorded comparisons](global-validation.json)

Global blending remains incomplete for three or more profiles. The current model
blends neighboring refitted profiles linearly in arc length. Fresh Rhino 8.32
captures demonstrate that this differs from both the SDK and actual commands.
McNeel's [Sweep1 help](https://docs.mcneel.com/rhino/8/help/en-us/commands/sweep1.htm)
describes tapering between end profiles; that description alone does not specify
the multi-profile interpolation rule.

The 18 complete surface definitions cover three/four profiles, offset profile
endpoints, explicit refit and implicit refit with `RefitRail=No`, uniform scaling,
and translation. Two Local captures and three two-profile Global captures are
passing controls. In the recorded run, their complete images agree within
`2.52e-15`. The other 13 cases have certified image-gap witnesses. These results
do not qualify general Global blending or the separate spatial refit clock.

Matching positive V weight rows make every captured surface affine in normalized
V. Bounds on both complete U boundary curves therefore extend to the entire
surfaces under the same parameter correspondence. For gaps, numeric searches
propose a boundary witness and target UV. Exact source evaluation proves witness
membership within `1e-12`; a complete point-to-surface distance certificate then
bounds its distance to the other surface within `1e-9`. A lower bound above
`1e-6` proves different images. A sampled discrepancy alone is insufficient.

The regression records the current classification, so a future correct blend can
replace a gap with a complete-image proof. It requires the Local/two-profile
controls to keep passing. Capture producers and requests are frozen; validation
inventories bind the Rust test and the certificate implementation. Preliminary
probes found differences for piecewise-linear, natural-cubic, not-a-knot and
clamped cubic interpolants of the supplied profile control positions. These
comparisons leave the replacement multi-profile blend law unspecified.

```sh
cargo test -p viboceros-geometry --test sweep_global_shapes --frozen -- --nocapture
python3 -m unittest tools.rhino_oracle.test_sweep1_global_shapes
```
