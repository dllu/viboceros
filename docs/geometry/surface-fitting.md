# Polynomial surface fitting

[Geometry guides](README.md) · [Surface bounds](surface-correspondence.md)

[Automatic bounded fitting](surface-fitting-automatic.md) chooses U/V knots under a complete source bound.

`try_interpolate_surface_in_basis(source, degree, knots)` samples Cartesian
positions at the target tensor's Greville stations and produces polynomial
controls. It retains the original UV domains, supports unequal degrees and
nonuniform knots, and can approximate rational sources. The input is immutable.
Each axis uses one factorization for all XYZ columns. Local coordinates,
constant-channel restoration and exact corners reduce avoidable roundoff.

`try_fit_surface_in_basis(source, degree, knots, limit)` additionally checks
the complete original surface with the exact tensor correspondence bound.
It returns a `BoundedSurfaceFit` with the certified maximum displacement, or
`None` for an outside or inconclusive proposal. Zero is strict. Interpolation
stations alone never authorize a bounded result. Trimming, topology and tangent
continuity are outside this certificate.

The target must have clamped ends, degrees 1..11, interior multiplicities at
most the degree, and strictly increasing Greville stations. Each target axis
allows at most 64 controls. Sources require degrees at most 16, at most 4096
controls, and nonzero common-sign weights. Invalid bases, resource limits and
malformed backend results fail before any partial output is returned.

The `_with_solver` forms share sampling and admission with external solvers.
`viboceros_smlib::{interpolate_surface_in_basis, fit_surface_in_basis}` uses
usd-brep's batched collocation, adapting Cartesian columns to the native
four-channel layout.

## Rhino evidence and remaining work

A headless Python `surface_fit` probe calls the public
[Surface.Fit API](https://developer.rhino3d.com/api/rhinocommon/type/rhino.geometry.surface/).
Its 36 observations cover planar, curved, rational, piecewise-linear and
redundant tensors, degree changes, and three tolerances. Thirteen return null.
The Rust comparison supplies each of the 23 successful outputs' **captured
bases** to the independent fitter and checks the complete result at `1e-9`,
including a 3DM round-trip. This qualifies tensor controls in those bases.

Rhino's adaptive knot selection and the `FitSrf` command remain unfinished. In
particular, a native fit may change planar parameterization; an unproved
normalized parameter bound does not establish geometric disagreement.
The [planar image certificate](surface-planar-images.md) and
[physical command captures](../surface-fit-command-investigation.md) address
these distinct questions without changing fitting admission.
The [validation record](../surface-fit-basis-validation.json) keeps native null
results and declined source-correspondence bounds separate from basis matches.

```sh
cargo test -p viboceros-geometry --lib surface_fit::tests
cargo test -p viboceros-oracle --test surface_fit_basis_native
cargo test -p viboceros-oracle --features native-smlib --test surface_fit_basis_native
python3 -m unittest tools.rhino_oracle.test_surface_fit
```
