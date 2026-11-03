# Closed-edge Inset networks

[Curved boundaries](inset-curves.md) · [Edge partitions](inset-partitions.md)

With `Loose=Off CreaseCorners=Yes`, a tangent-continuous closed NURBS edge can
produce several border cells. Curvature breaks in the fitted cap generate
connectors from the original boundary vertex. Connectors split one another and
the source boundary wherever they cross. Shared edges retain their original
geometry and are subdivided in every incident trim's parameterization.

An expanding stadium-shaped hole supplies a crossing control: the connector
from `(4,2)` to `(-4,-3.5)` meets the source bottom at `(-20/11,-2)`.
Its six-face result has 13 edges and nine vertices. If an unselected coplanar
face fills the hole, connector chords also partition that face; the joined
result has nine faces, 15 edges and nine vertices. Every face retains its
original supporting surface.

The stored source seam controls the border layout. A seam at a G2 break adds a
connector. A seam inside a straight span remains a cap vertex without adding
a connector there. A smooth curved seam retains an annular border. Internal
cap tangent kinks use the native one-degree threshold, independently of model
angular tolerance; existing source edges keep their stored internal turns.
These rules describe measured native behavior rather than invariance under
moving a closed curve's seam.

The tight fitter retains the source degree and refines its knot basis using
homogeneous Greville interpolation. A banded solve avoids dense storage.
Normal-distance samples control refinement to model tolerance; these samples
are numerical checks, not a continuous error proof. Admission allows degree 17,
1000 fit controls, 8192 network inputs/fragments, and the shared 64 MiB Inset
output budget. Supporting surface copies are charged before construction.

Joined output shares the complete cell topology. Separate output cuts generated
edges between components while retaining source-edge fragments as connections.
Curves output returns the fitted cap loops and unsplit connectors, together
with the unchanged source object. The adjacent-face control yields six separate
pieces: three with one face and three with two faces.

Frozen private-display comparisons cover 20 SDK cases: 14 valid results and six
native nulls. These closed-edge controls use quadratic source curves; analytic
fitter tests additionally cover degrees 3, 7 and 17. Six successful commands exercise all three output modes, metadata,
selection, Undo/Redo and 3DM roundtrips. Three additional native command controls
record cancellation when the adjacent polysurface has no selected face components.
Comparisons require matching oriented topology and corresponding-edge witnesses
within `2e-6` at `1e-6` model tolerance. Portable and usd-brep paths share the
network assembly.

Creased cusp fans, adjacent selected coplanar networks, curved G0 corners and
offsets that change the retained cap topology remain unsupported. Errors leave
the source unchanged. [Fan diagnostics](../geometry/inset-fans.md) retain the
pending native cases.

```sh
cargo test -p viboceros-oracle --features native-smlib --test inset_surface closed_curves
python3 -m unittest tools.rhino_oracle.test_inset_closed_networks
```
