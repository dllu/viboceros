# Bend live preview

[Command guide](commands/bend.md) · [Geometry](bend-geometry.md)

After accepting both spine points, mouse movement previews the bend in every
viewport. Selected sources stay in place, including their shaded faces. The
pending deformation uses object-colored wires; accepting the through point runs
the command's full fit. Copy placements keep the original sources for subsequent
previews. Preview movement creates no model edits or history entries.

Bend's free mouse plane is the viewport construction plane translated through
the **spine start**. Native Top captures with starts at z=0 and z=3 distinguish
this from the spine-end plane. Explicit snaps and point filters retain the common
drafting path. Typed coordinates retain their existing command semantics.
A cursor on the spine removes the pending deformation; leaving a viewport keeps
the last valid point shared by the other views.

The construction guide shows the circular spine region and its tangent extension,
with markers at the original spine endpoints. A positive numeric Angle controls
the circular extent. Saved numeric angles affect preview and completion equally;
Angle=0 restores through-point construction.

## Display preparation

Bend and Twist share `src/viewport/morph_preview.rs`. Immutable source snapshots,
wire density, top groups and document tolerance identify prepared cages. Curves
use degree-at-least-three controls. Low-degree polynomial surfaces use cached
interpolation grids; preserved surfaces move their existing controls. B-reps
prepare edge cages and trimmed isocurve intervals. Meshes map cached wire
endpoints, and rigid objects reuse source display geometry with an affine pose.
No full geometry fit runs on cursor movement.

Bend shows surface and B-rep interior isocurves in Wireframe. Shaded and Ghosted
pending objects show boundaries. Both variants are cached together, so viewports
with different display modes share one morph evaluation. Source faces keep their
original appearance. Copy and object-color changes do not rebuild geometry;
geometry, tolerance, wire-density and group changes invalidate the cache.

This display approximation follows the measured public quick-preview behavior.
[RhinoCommon's QuickPreview documentation](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.spacemorph/quickpreview)
allows preview morphs to ignore model fitting tolerance. Preview cages are not
accepted model geometry or a tolerance certificate.

## Native evidence

Private Xvfb captures from licensed Rhino **8.32.26160.13001** retain 35 owned
cursor cases: points, lines, curves, surfaces, solid boxes and colored meshes;
all Bend flags, Copy/repeat/cancellation, Front/Perspective/Top projections,
short bends, and a translated spine. Files are under
`tools/rhino_oracle/{fixtures,observations}/bend_preview*.json`; raw PNGs and
[provenance hashes](bend-preview-provenance.json) identify the evidence.

Rust tests compare public SDK preview controls and curve samples at `1e-11`,
weights at `1e-14`, rigid placement at `1e-7`, source preservation, cached scenes,
mouse picks, invalid-point removal, point-filter suspension and option memory.
The egui pointer/camera round trip uses `1e-5` because screen coordinates use
single precision. Python tests check capture ownership, private-display guards,
PNG hashes, terminal state and projected wire pixels. Curve witnesses use three
pixels, surface boundary witnesses two, and Wireframe isocurves four.

Two diagnostic captures remain separate from the verified cases. The initial
27-case capture exposed a first-Copy screenshot taken before native motion;
the picker now waits for native acknowledgement before collecting pixels. The
initial seven-case follow-up computed Top calibration through the spine end;
the corrected capture uses the spine start. Geometry and native pixels in those
diagnostics are retained, with only PNG transport paths renamed.

These finite witnesses do not prove exhaustive viewport parity. Arbitrary
construction planes, complex trim loops, singular maps and all combinations of
drafting aids still need native coverage. Point glyphs use the application's
common drawing style.

```sh
cargo test -p viboceros --bin viboceros viewport::bend_preview
cargo test -p viboceros --bin viboceros app::tests::bend
python3 -m unittest tools.rhino_oracle.test_bend_preview
```

Verification: Bend, Twist and affine cursor regressions passed, along with 31
related Python tests. The shared deformation overlay GPU check passed in linear
and sRGB formats on NVIDIA GB10/Vulkan, driver 610.57.04. Formatting and application
all-target Clippy passed; Clippy retains existing warnings elsewhere.
