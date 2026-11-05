# SubD area and area centroids

[Measurements](../commands/measurements.md) · [AreaCentroid](../commands/area-centroid.md)

The two `subd_area_*` families retain 19 owned SubD workflows and 38 actual Rhino
Area/AreaCentroid commands. They cover regular and curved grids, hard creases,
extraordinary vertices of valence 3/5/7/8, a nonquad face, a closed box, flat and
smooth appearance, mixed curve inputs and command-first selection. Captures run
under private Xvfb with an isolated Rhino settings scheme. Successful responses
and content-addressed producer snapshots are immutable; compression preserves
all original bytes.

Flat appearance does not change these area queries. Public `ToBrep` diagnostics
compare five extraordinary-vertex modes separately from actual command outputs.
Seven- and eight-valent asymmetric controls distinguish Local G1x from Local G1
by more than `0.001` square model units; actual commands match Local G1x. Choosing
G0, G1 or the control cage would produce different measurements.

Rust constructs the source recipes independently, prepares Local G1x patches
once per query, then reuses NURBS area or area-moment quadrature. No temporary
document objects or display meshes participate. Regular neighborhoods retain
exact bicubic patches. Extraordinary neighborhoods and soft creases retain the
existing finite conversion approximation, without a continuous error certificate
against the mathematical subdivision limit. Existing conversion and per-patch
quadrature work limits apply. The document absolute tolerance is divided across
prepared patches; failures produce no partial report or marker.

Command replays compare selection, marker count, coordinates, source preservation
and seeded Undo/Redo cycles. Printed command areas use absolute `1e-6`; tight
public Local G1x area diagnostics and actual centroid coordinates use `1e-8`.
At those unchanged thresholds, 36/38 command comparisons pass. The two mixed
SubD/circle centroid commands retain default-native coordinate residuals of
about `1.15e-8` and `4.00e-8`. Separate checks against tight public SubD mass
combined with analytic circle area pass at `1e-8`; these do not replace or count
as passes for the two default-command comparisons.
Native integration estimates and printed rounding are retained in raw histories.
App tests cover flat/smooth preselection, mixed inputs and command-first picking.
Analytic planar controls also exercise display units, marker history and atomic
failure. General performance parity, volume queries and nested face-component
measurements are not established by this corpus.

The release Python replay preserves the strict default comparison and exits 1
for those two residuals. Its explicit regression gate checks the known residual
set, all structural states and the separate tighter references:

```sh
python3 -m tools.rhino_oracle.subd_area_replay
python3 -m tools.rhino_oracle.subd_area_replay --check-known
cargo test --release -p viboceros-oracle --test subd_area
```
