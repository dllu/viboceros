# Fitted-plane orientation diagnostics

[Circle fitting](circle-fit-conditioning.md) · [Maelstrom FitPoints](maelstrom-fit-points.md)

The existing 44 private Xvfb Circle/Plane SDK captures show identical plane
axes in both public fitters. Circle loci can match while their oriented normals
or seams differ. That distinction affects signed Maelstrom deformation and
parameterized curve comparisons.

The independent [diagnostic example](../crates/viboceros-geometry/examples/inspect_plane_basis.rs)
compares faer and nalgebra decompositions with those public outputs. It uses
unscaled coordinates and either no centering or a sequential arithmetic mean.
This is separate from the kernel fitter, which uses correctly rounded centroid
sums, scaling and a thin SVD. No experiment here is installed as a replacement
native-compatible plane algorithm.

## Matrix and normal experiments

For each point set, the example decomposes rows `(x,y,z)` or `(x,y,z,1)`. The
normal is the XYZ part of the last right singular vector. For three-column
matrices, another variant takes the cross product of the first two right
singular vectors. `Frame3::try_from_normal` supplies the deterministic plane
axes. The fourth-column variants test homogeneous plane fits; they do not
generally minimize the centered spatial-distance objective.

Matching requires Euclidean normal-vector error at most `1e-12`. A complete
basis match also requires both X/Y axis errors at most `1e-12`. The table counts
oriented matches, without permitting an opposite normal or rotated seam.

| Solver | Centered | Columns | Normal source | Oriented normal matches | Complete basis matches | Available records |
| --- | --- | --- | --- | ---: | ---: | ---: |
| faer | No | 3 | Last V vector | 23 | 20 | 44 |
| faer | No | 3 | First two V vectors crossed | 24 | 22 | 44 |
| faer | No | 4 | Last V vector | 22 | 17 | 44 |
| faer | Yes | 3 | Last V vector | 27 | 23 | 44 |
| faer | Yes | 3 | First two V vectors crossed | 29 | 24 | 44 |
| faer | Yes | 4 | Last V vector | 28 | 24 | 44 |
| nalgebra | No | 3 | Last V vector | 17 | 15 | 44 |
| nalgebra | No | 3 | First two V vectors crossed | 16 | 14 | 44 |
| nalgebra | No | 4 | Last V vector | 13 | 3 | 40 |
| nalgebra | Yes | 3 | Last V vector | 18 | 15 | 44 |
| nalgebra | Yes | 3 | First two V vectors crossed | 16 | 12 | 44 |
| nalgebra | Yes | 4 | Last V vector | 16 | 8 | 40 |

nalgebra's four-column diagnostic skips the four three-point inputs. A zero
XYZ normal records `null` rather than manufacturing a frame. The 44 records
include two degenerate circle inputs with public Plane status `Success`;
their recorded bases are witnesses, not unique geometric plane definitions.

No variant matches every available record. Floating-point decomposition,
centering, ordering and sign decisions remain possible sources of the
differences; these experiments do not identify proprietary implementation.
The [raw diagnostic output](plane-fit-basis-diagnostics.json) and
[provenance record](plane-fit-basis-provenance.json) retain every measurement
and input hash rather than choosing an algorithm from its match count.

## Plane origin and geometry checks

In all 44 captures, the native plane passes through the point centroid but its
stored origin is the nearest point on that plane to world zero. Both measured
residuals are below `1e-12`. This origin convention is independent of the
Circle's fitted center and the unresolved oriented basis.

Circle regression checks compare unoriented normals by
`min(|n-native_n|, |n+native_n|) <= 1e-12`, then measure 64 native boundary
witnesses against each accepted fitted circle. This catches angular errors
amplified by large radii. Regular boundary distances use `1e-7`; large circles
use the larger of `1e-7` and sixteen native-radius ULPs. The known `1e-6`
near-collinear center/radius gap stays explicit and is excluded from passing
locus comparisons.

```sh
cargo run --release -p viboceros-geometry --example inspect_plane_basis \
  > /tmp/plane-fit-basis-diagnostics.json
cargo test --release -p viboceros-geometry circle_fit
python3 -m unittest tools.rhino_oracle.test_plane_fit_basis
```

The example reads retained public output files and never launches Rhino.
Regenerating those native inputs must use `run_headless.sh` with a private
settings scheme and Xvfb display.
