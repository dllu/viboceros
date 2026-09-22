# Surface volume first moments

[VolumeCentroid](commands/volume-centroid.md) · [Earlier open-boundary audit](volume-centroid-open.md)

`VolumeCentroid` explicitly uses coordinate-direction primitives for surfaces
and B-reps, and tetrahedral cones for meshes, matching the observed Rhino
command. The geometry kernel's default remains **uniform cone integration**.
This distinction matters even for an enclosing collection if its boundary mixes
open mesh and surface pieces; command compatibility is not a physical-volume
certificate.

## Independent derivation

Let `q = S - b`, with surface Jacobian normal `n`, and common reference `b`.
Both conventions use `V = integral(q dot n)/3`. Their first moments are:

```text
cone:       M_j = b_j V + integral(q_j (q dot n))/4
coordinate: M_j = b_j V + integral(q_j (q dot n) - q_j² n_j/2)/3
```

For the second formula, the vector field's j-th component is `q_j²/6` and each
other component is `q_j q_i/3`. Its divergence is `q_j`; the radial cone field
also has that divergence. The two therefore agree on a consistently oriented
closed boundary, but need not agree on an open piece. This is an independently
derived mathematical identity, not a translation or inspection of proprietary
integration code.

The [public surface API](https://mcneel.github.io/rhino-cpp-api-docs/api/cpp/class_o_n___surface.html)
requires a common base when accumulating separate boundary pieces. McNeel's
[public centroid sample](https://developer.rhino3d.com/en/samples/cpp/calculate-volume-centroid-of-solids/)
chooses the combined bounding-box center. Our inference about the first-moment
density is supported by the captures below, not specified by those API pages.

For `S(u,v)=(4u,3v,2uv)` and `b=(2,3/2,1)`, volume is −2. Cone first moments
are `(-4,-3,-4/3)`, giving centroid `(2,3/2,2/3)`; coordinate primitives give
`(-10/3,-5/2,-2)` and centroid `(5/3,5/4,1)`. The latter reproduces Rhino's
command and collection API. At `b=(2,3/2,1/2)`, the coordinate integral instead
has zero volume and first moments `(2/3,1/2,-1/2)`, explaining the earlier
single-object API residual. That example does not establish a general base
selection rule for every single-object API overload.

## Mixed-boundary counterexample

Six outward faces enclose a 4 × 3 × 2 box. With all faces represented as surfaces,
or all as meshes, both Rhino and uniform cone integration put the centroid at
`(2,3/2,1)`. Representing just the bottom face as a mesh changes Rhino's actual
command and collection-API result to `(2,3/2,23/24)`. The true box center has not
changed. A mesh top face instead gives `z=25/24`; rotated variants retain the
mixed-convention bias.

Each convention is valid on a closed boundary when used consistently. Mixing
different vector fields across its pieces prevents the divergence-theorem
cancellation. Consequently the geometry API exposes the choice explicitly:

- `VolumeMassProperties::from_boundaries` and `volume_flux` retain uniform cones.
- `SurfaceVolumeMoments::CoordinatePrimitives`, through
  `from_boundaries_with_surface_moments` or surface/B-rep
  `volume_flux_with_moments`, selects the alternative surface density.
- `VolumeCentroid` requests that surface convention for Rhino command parity;
  scalar `Volume` is unchanged. Strict single-solid kernel methods remain cones.

No case ID, observed coordinate, or fixture-specific adjustment participates in
the native implementation. Existing independent mixed-box tests still require
the physical centroid from the default kernel API.

## Evidence and capture failure

The [source generator](../tools/rhino_oracle/references/volume_surface_primitives.py)
builds 36 cases: two bilinear surface families, genuine non-axis rotations,
shears, translations, changed shared references using cancelling mesh shells,
multiple surfaces, and pure/mixed unjoined boxes. The [independent reference](../tools/rhino_oracle/references/volume_primitives_integrals.py)
symbolically integrates polynomials using exact `Fraction` arithmetic on the
binary64 inputs. Its [rational witnesses](../tools/rhino_oracle/fixtures/volume_surface_primitives_reference.json)
read no native results or observations. They reproduce all 36 actual command
centroids within `1e-9` (maximum `3.56e-14`); the uniform-cone hypothesis matches
only six of these deliberately discriminating cases.

The [native command replay](volume-surface-primitives-comparison.json) also
matches all **36/36**, with maximum coordinate difference `3.56e-14`.
[Regression replays](volume-surface-primitives-regression.json) give **42/42**
for the original warning suite and **26/26** for closed centroids. The earlier
80-case scalar/centroid suite improves from 71/80 to **77/80**; only its three
existing scalar discrepancies remain. Its six surface-centroid differences
are resolved, without changing any observed target or comparison tolerance.
See [provenance and hashes](volume-surface-primitives-provenance.json).

The initial 36-case session suffered a runtime access violation while recording
a rotated face, before publishing any response. The owned worker process was
confirmed absent before its still-waiting host was interrupted. That
[failed attempt](../tools/rhino_oracle/observations/volume_surface_primitives_attempt.txt)
is not counted as a command result. Five fresh, smaller owned private-Xvfb
sessions completed batches of 8, 8, 8, 8 and 4 cases. Their concatenated
[complete observations](../tools/rhino_oracle/observations/volume_surface_primitives.json)
preserve every numeric value bit-for-bit; no failed or missing case is replaced
by an expected target. Existing user Rhino sessions were untouched.

The client now detects a genuinely departed, previously observed worker after
its progress file exists, and rechecks the atomic response before reporting an
early exit. A silent but live worker, or a process not yet observed, is not
treated as terminal. This does not repair or explain the runtime access violation.

The new captures establish the tested polynomial families, not all rational or
trimmed open-surface configurations. Closed curved-solid and extreme-parameter
tests remain separate checks. No new Rhino performance-parity claim is made.

```sh
python3 -m tools.rhino_oracle.volume_centroid_replay \
  tools/rhino_oracle/fixtures/volume_surface_primitives.json \
  tools/rhino_oracle/observations/volume_surface_primitives.json
python3 -m unittest tools.rhino_oracle.test_volume_primitives
```
