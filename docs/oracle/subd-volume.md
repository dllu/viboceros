# SubD volume and volume centroids

[Volume](../commands/volume.md) · [VolumeCentroid](../commands/volume-centroid.md)

The three `subd_volume_*` families retain 58 actual Rhino commands: 36 closed-shell
controls, 18 open-shell warning workflows and four replacement controls. Sources include smooth, creased,
sharp, inward, translated and scaled boxes, an asymmetric seven-valent bipyramid,
open boxes, a bent grid and mixed closed meshes. Flat/smooth appearance and
command-first selection are recorded. All captures use private Xvfb and an
isolated Rhino settings scheme. Successful observations and frozen producers
are immutable; gzip preserves their complete original bytes.

Separate public `ToBrep`/mass-property diagnostics distinguish Local G1x from
other extraordinary-vertex modes. In the seven-valent control, Local G1 and
Local G1x volumes differ by more than `0.1` cubic model units. Actual commands
follow Local G1x. Display appearance does not change the measurements.

Six inward-input controls record a complete reversal on native document
insertion, with unchanged control coordinates and tags. Viboceros retains the
public OpenNURBS solid-orientation witness in immutable SubD records and reverses
only known inward solids during admission. Replacement controls confirm the
same policy for replaced closed shells while preserving reversed open shells. Open inputs and unknown orientation
retain their supplied sense. Raw readers/kernel values stay lossless; reversing
a private archive copy preserves source bytes and native component IDs. No mass
quadrature or per-component orientation guesses determine admission.

Volume boundaries prepare each SubD's sewn Local G1x B-rep once. Shared bounds
and reference points are chosen after preparation, including mixed SubD/mesh
collections. Scalar queries skip first moments and retain exact cross-object
aggregation and unit conversion. Centroid commands retain the explicit surface
coordinate-primitive convention. Open pieces remain reference-dependent fluxes,
requiring the existing warning; acceptance does not certify an enclosed solid.

All 20 native warnings have recorded PID-owned dialog delivery. Yes and Escape
continue; No declines. Replays compare input/admitted definitions, selection,
source preservation, marker coordinates and seeded Undo/Redo cycles. The actual
command thresholds are absolute `1e-6` for printed volumes and coordinates;
separate tight public Local G1x volume and centroid checks use `1e-8`. Analytic
sharp-box tests also check signed raw kernel geometry, display units, immutable
reversal, admission and atomic warning rejection. App tests drive picking and
warning answers without opening windows.

Regular patches are exact bicubics. Extraordinary neighborhoods and soft creases
retain the existing finite conversion approximation and work limits, without a
continuous error certificate against the mathematical limit. Conversion or
quadrature failures abort the complete query. General performance parity and
face-component volume measurements are not established by this corpus.

```sh
python3 -m tools.rhino_oracle.subd_volume_replay
cargo test --release -p viboceros-oracle --test subd_volume
python3 -m unittest tools.rhino_oracle.test_subd_volume
```
