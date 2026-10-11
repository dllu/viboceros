# Native curve extrusion

Linux builds with `--features native-smlib` now use usd-brep for ExtrudeCrv's
geometric construction. The command's parser and document policy live in the
separate `extrude_curve` module. Native builds exclude the previous direct Rust
extrusion constructors from this command; other builds retain them.

Sheets use `SmApiCreateExtrude`, with the existing native `SwapUV` and
`Reparameterize` operations to retain profile U and physical-distance V.
Standalone surfaces reuse the B-rep bridge's NURBS payload writer and decoder,
with a separate owned surface snapshot. No native pointer enters document data.
BothSides translates the profile to the start offset and sweeps to the end.
Open or nonplanar profiles remain sheets when Solid is requested.

Capped planar profiles use the existing `SmPrimitiveCreation` linear-sweep
producer underlying SM_API. Its owned B-rep receives the document tolerance
before construction: the high-level API has no creation-tolerance argument.
Native curve copies are consumed by the producer, including on failure; source
handles remain owned and usable. Caps must cross the profile plane and produce
a native manifold solid, followed by the shared Rust B-rep validation.
The bridge also exposes multiple-profile sweeps, tested with an annular solid;
the command continues to create independent outputs for independent objects.

Unclamped profiles use the [native canonical constructor](smlib-curve-import.md)
to clamp their active interval without changing its parameterization. A uniformly negative
weight gauge is flipped as one homogeneous gauge before native import. Original
objects remain unchanged. Mixed-sign weights and discontinuous interior knots
remain adapter restrictions. Changes to parameterization or topology in capped
outputs are not qualified as raw Rhino parity.

All outputs are staged before document mutation. Current-layer output attributes,
source retention/deletion, group removal, selection and Undo/Redo retain the
existing command flow. A later unsupported profile rejects the entire batch and
preserves the previous redo entry.

The [three live Rhino command records](../tools/rhino_oracle/observations/curve_native_extrusion.json)
cover native polylines and custom line/polyline domains. They disable Rhino's
CreaseSplitting for the sheet comparison. Every raw NURBS definition field agrees
at `1e-10` absolute / `1e-12` relative epsilon, without parameter or control-net
normalization. See the [comparison and retained native response](smlib-extrusion-comparison.json).
The first private-Xvfb attempt timed out before returning results; the second
completed using the same private settings scheme. No shared desktop was used.

Independent regressions check signed/oblique rational-sheet samples, parameter
domains, native curve preservation, positive/negative capped-circle volume,
annular volume and cap holes, degenerate/coplanar rejection, unclamped and
negative-gauge sources, and command history/metadata. These checks qualify those
fixtures. General self-intersection, tiny/huge scales, arbitrary imported
topology and Rhino performance parity remain open. Native sweep calls accept at
most 128 profile handles. See the [limitations ledger](usd-brep-limitations.md).

Release validation passed 1,306 native command tests and 47 native bridge tests,
with two command tests and one bridge test ignored. Six default-backend
extrusion regressions passed. Native clippy with warnings denied and workspace
formatting checks passed.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test extrusion
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib extrude_curve
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/curve_native_extrusion.json --timeout 360 \
  --scheme VibocerosOracleUsdExtrude20261011 \
  --output tools/rhino_oracle/observations/curve_native_extrusion.json
```
