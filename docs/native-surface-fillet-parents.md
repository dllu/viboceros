# Fillet parent trimming and splitting

`fillet_surfaces_with_parents` extends the native rolling-ball producer with
owned trim or split results for its two support surfaces:

```rust
use viboceros_smlib::{
    SurfaceFilletOptions, SurfaceFilletParentMode, fillet_surfaces_with_parents,
};
let options = SurfaceFilletOptions::try_from_picks(
    &first, &second, [[u0, v0], [u1, v1]], radius, false, tolerance,
)?;
let result = fillet_surfaces_with_parents(
    &first, &second, options, SurfaceFilletParentMode::Trim, tolerance,
)?;
```

`None` means no fillet was produced. A successful result contains the fillet
and two optional parent replacements. For each parent, `None` means retain the
original; `Some(faces)` replaces it with separate owned faces. An empty
replacement reports complete native consumption. The caller's input geometry
remains unchanged on success and failure. Replacement parents use world
coordinates and now retain their [original UV charts](fillet-parent-charts.md);
construction still uses private normalized inputs. The API accepts two
NURBS support surfaces; it does not import existing trimmed parent topology.

The implementation reuses usd-brep's rail insertion and base-surface trimming.
Its face deletion requires editable parent bodies; the adapter enables editing
only on the private copies used for trimming. Split mode exports the imprinted
faces separately. The fillet-only entry point retains its existing behavior.

An open contact rail wholly inside a parent cannot split that parent's region.
The kernel can leave an embedded antenna and an isolated point loop there.
The adapter removes only nonclosed, embedded struts with the kernel's deletion
operations, then removes their newly isolated vertices. Original vertices,
periodic seams and closed contact curves are excluded from this cleanup.
Comparing the remaining topology to the private original distinguishes an
unchanged parent from an empty replacement. An unchanged plane is retained
instead of being deleted when the public SDK reports no replacement for it.

The oracle's optional `trim` field selects the public
[RhinoCommon parent-result overload](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateFilletSurface_1.htm):
`true` trims, `false` splits, and an absent field uses the existing fillet-only
overload. Explicit nulls and non-booleans are rejected. Complete fillets and
both parent-result groups retain their definitions, topology, bounds, areas
and closest queries. Source-purity checks include the SDK parent topology.

Thirty-two fresh SDK recipes cover both modes, four plane quadrants,
inner/outer/boundary cylinder picks, U/V reversals and transposes, rational
parameter changes, and a reversed first plane. Parent comparisons allow the
SDK and native split regions to appear in different orders. The region helper
qualifies these rectangular planar/cylindrical fixtures, checking their complete
trim coefficients within `1e-12` UV of rectangle sides before reconstructing the
regions for sampled membership checks. Raw trim coefficients and result order
remain unchanged in the complete protocol audit. This helper does not decode
or qualify arbitrary curved trim loops. Independent bridge tests check analytic
areas, picked regions, a three-face joined result, extreme spatial scales and
signed weight gauges.

A full-circle plane/cylinder recipe still exceeds the existing surface/curve
certificate work limit. The retained `dd679da0` executable rejects the same input.
The new API preserves that guard and both sources. Native closed topology can
be inspected by the C ABI probe, but successful native construction does not
establish certified Rust B-rep transfer. Curved `Extend=Yes` matching also
remains unfinished; this change retains its existing policy.

The initial [FilletSrf command](commands/fillet-srf.md) now supplies interactive
picks, previews, metadata preservation and undo/redo. General trimmed parent
inputs, disconnected-component choice and remaining extension/certificate work
are still in progress. [Validation evidence](native-surface-fillet-parents-validation.json)
records the actual gates, complete raw comparisons, native ownership probes
and outstanding limitations. AddressSanitizer leak detection is disabled.

```sh
cargo test --release -p viboceros-smlib --features native --test surface_fillet_picks
cargo test --release -p viboceros-oracle --features native-smlib --lib surface_fillet
python3 -m unittest tools.rhino_oracle.test_surface_fillet_probe
```
