# ScaleByPlane Object targets

[Command usage](commands/scale-by-plane.md) · [Provenance](scale-by-plane-object-provenance.json)

The bounded `scale_by_plane_object.json` request records 64 public recipes in
Rhino 8.32.26160.13001 on private Xvfb. Every recipe starts in an empty owned
document at idle and creates four source points and a separate plane target
in a public Undo record.

Targets include lines, polylines, rational cubic curves, NURBS representations
of circles/arcs/ellipses, rectangular and skew planar surfaces, warped bilinear
surfaces, nonplanar rational curves, points, and whole quad meshes. World and
tilted target frames and reversed curve/surface parameter directions are
prescribed. Eight cases use Copy; additional point and mesh recipes start with
a tilted active construction plane.

The probe separately runs public `CPlane Object` and records its frame. It then
restores the prescribed active CPlane and runs `ScaleByPlane Plane=Object`.
The four source points distinguish independent directions of the affine map;
their output is compared with signed projection ratios using the separate
CPlane witness. No output coordinates are fitted into the implementation.
Public SDK `TryGetPlane` results distinguish planar and nonplanar geometry.

## Measurements

- 56 targets are accepted, including nonplanar curves, warped surfaces, and
  points. Nonplanar curve frames use start tangent and curvature; surface
  frames use U/V derivatives at the parameter midpoint.
- Point targets retain the active CPlane axes, including when it is tilted.
- Eight whole-mesh ID selections are rejected. Their geometry remains unchanged.
  This does not establish the behavior of mesh face picking.
- Accepted commands retain names and source order, clear selection, and support
  external Undo/Redo. Copy preserves the four sources and creates four outputs.

Rejected mesh input returns to the origin getter after a macro Cancel. The
probe waits for public SelID completion, requests Escape in its owned window,
and records the ScaleByPlane EndCommand result and the driver's input receipt.
The driver accepts only the requested mesh cancellation marker. A timeout or
incomplete sequence cannot become a capture.
All input is confined to private Xvfb.

Application tests compare each accepted recipe as complete input and incremental
picks (112 replays), plus eight rejected target pick/cancel replays. They compare
all source point coordinates, names, object order, selection, Copy, and external
history at absolute epsilon `1e-9`. Rejected complete commands are also checked
for atomic failure. The plane target must remain unchanged.
A separate application regression verifies that repeated Object copies keep
using the saved sources after selection clears, and share one Undo entry.

These recipes do not measure trimmed/multiple-face B-reps, other curve families,
mesh face picks, Rigid groups, previews, extreme coordinates, or performance.
Full native parity remains unproven.

## Reproduce

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/scale_by_plane_object.json --scheme VibocerosOraclePlaneObject --output tools/rhino_oracle/observations/scale_by_plane_object.json --timeout 360
python3 -m unittest tools.rhino_oracle.test_scale_by_plane_object
cargo test --release --bin viboceros scale_by_plane
```
