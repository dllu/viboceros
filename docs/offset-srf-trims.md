# Trimmed surface offsets

Native `OffsetSrf Loose=No Solid=No` offsets the underlying surface of a single
trimmed face, then retains its UV boundaries and holes. Reversed faces use the
opposite signed distance. Separate `BothSides` skins use the same path.
Spatial trim images are rebuilt with continuous surface/curve certificates;
piecewise axis-aligned UV trims preserve their parameter clock and rational
isocurves. This avoids transferring inconsistent native edge/UV parameterizations.
The support-only native fit uses one quarter of the requested tolerance; the
separate trim-image certificate uses one quarter of document tolerance.
The UI retains two prior valid prepared candidates, revalidates their sources
before reuse, and reapplies current `DeleteInput` admission flags. Returning to
an earlier distance or Solid setting can reuse its certified geometry.

Two new licensed Rhino 8 captures ran on private Xvfb displays. Qualified cases
cover negative distance, reversed orientation, both sides, finer tolerance,
different trim extents, batches, postselection with remembered `DeleteInput`, and
cancellation. `DeleteInput` remains ineffective for open offsets.
The failed import attempt and hidden-DeleteInput diagnostic remain in evidence;
the latter is repeated with the option set while visible. Earlier tolerance
captures remain immutable. Command and UI checks include source identity,
metadata, selection, preview purity, actual Undo/Redo and 3DM round trips.

Finite face and UV-trim-image comparisons use `max(1e-5, 2*requested)` model units
for curved refits; native model-space edge parity is not separately qualified.
Independently displaced source-normal stations require product residuals within
the requested tolerance plus `1e-10`; native residuals are recorded separately.
Reference B-reps use the larger of document tolerance and requested fit tolerance
for admission: Rhino's edge/UV fits can exceed document tolerance. Product
validation retains document tolerance.
The trim-image certificate does not establish a continuous ideal-offset bound.
General trimmed solids, multiple support charts, singularities and comparative
performance are unqualified. This historical qualification retains the nonuniform rational
curvature-break input's atomic decline; its newer [patch-fitting path](offset-srf-rational.md)
has separate targeted checks. The trimmed repair also declines supports with G2
breaks until separate chart fitting is available. General curved offsets require
`native-smlib`.

[Qualification](offset-srf-trims-validation.json) ·
[Reproduce](../tools/rhino_oracle/check_offset_srf_trims.py) ·
[Tolerance behavior and earlier qualification](offset-srf-tolerance.md)
