# Bend geometry and oracle

`viboceros-geometry::BendPointMorph` implements a circular spine deformation,
independently derived from public Rhino SDK point maps and owned Bend commands.
The Python oracle exposes `bend_points` for comparing these maps. The application
command and interactive viewport adapter are still pending; this page documents
the geometry foundation, not a completed Bend command.

## Construction

`try_new(start, end, through, angle, straight, symmetric, tolerance)` follows
the public `BendSpaceMorph` constructors. The optional angle is in radians.
The through point determines the bend plane and radius. Without an explicit
angle, `straight` extends a shorter circular region through the spine end.
Symmetric bends reflect the deformation about the start plane. Outside the
circular region, the map continues along its terminal tangent.

`try_for_command` resolves the separately measured command parameters:

- LimitToSpine fixes circular arc length to the original spine length.
  With a positive explicit angle, radius is spine length divided by angle.
  Otherwise, the radius is adjusted so the arc's tangent extension passes
  through a target beyond the circular region. A target inside the circular
  region retains its through-point radius.
- An explicit zero angle restores through-point mode. SDK construction with
  a zero angle is invalid.
- `with_non_attenuated(true)` gives uniform circular stretch/compression.
  The default attenuation adds a cubic angular correction for points inside
  the center of curvature. It stops at and beyond that center.
- `rigid_transform(center)` places a group around its mapped bounds center
  using a circular tangent frame for uniform bends and forward derivative
  frames for attenuated bends. Five retained rigid captures cover both maps
  and off-spine centers.

Native SDK construction requires angle in degrees and circular arc length to
exceed `2^-32`; explicit angles above one full turn are invalid. Automatically
extended straight regions can span more than one turn. These construction
checks are separate from `try_from_arc`, which accepts a finite positive radius
and angle as a mathematical map.
A range fallback retains finite coordinates near the center of a bend even
when the ordinary radial calculation exceeds binary64 range; a regression
covers a radius of `1e308`.

`PointMorph` provides point clouds, mesh vertex maps and the existing adaptive
NURBS curve/surface fitters. `with_preserve_structure(true)` maps Euclidean
controls while retaining degree, knots, weights and domains. This control-cage
approximation does not promise a pointwise fitting tolerance. Generic fitting
has finite control/sample budgets and can reject singular or oscillatory maps.
Native curve/surface/B-rep representations, fitting policy and interactive
previews have not yet been compared for Bend.

## Captures and comparisons

All 124 operations were captured from licensed Rhino **8.32.26160.13001** in
owned settings schemes on separate Xvfb displays. No proprietary source was
inspected. Fixtures and raw responses are under
[`tools/rhino_oracle`](../tools/rhino_oracle/); the
[provenance record](bend-provenance.json) hashes every retained input and output.

The 76 SDK cases cover spatial/reversed spines, symmetry, straight regions,
positive/zero/negative angles, full turns, short arcs, validity boundaries,
through points on either side of the start plane, and points crossing the
center of curvature. Invalid SDK maps leave sample points unchanged.

The 48 command recipes retain owned source/after/Undo/Redo snapshots and
terminal events/history. Their 36 successful placements include LimitToSpine,
attenuation, explicit angles, Copy and rigid groups. Ten early angle macros
incorrectly used `Angle=value` for a prompt-only option; their failures remain
unchanged in `bend_command_followup.json`. Corrected `Angle value` captures are
in `bend_angle_command.json`, including two rejected negative-angle attempts.
The current helper uses the corrected syntax.

Ordinary point maps are compared at `1e-11`; large-radius and small-angle cases
use `1e-10` to allow native circle-center subtraction rounding. Rigid placements
use `1e-7`. These are finite sampled witnesses, not exhaustive compatibility
or a continuous error certificate.

The [release API comparison](bend-point-comparison.json) matches all 76 SDK
cases with a maximum coordinate difference of `2.61e-11`. The retained
[Rust timing response](../tools/rhino_oracle/observations/bend_points_rust_performance.json)
uses 1,000 iterations per case; valid batches measured about 39–60 ns per point,
including result collection. The native captures use public Python calls
through Wine/FEX, so their elapsed times do not establish a native Rhino kernel
speed comparison.

```sh
cargo test -p viboceros-geometry --lib bend::
cargo test -p viboceros-oracle --lib bend_points::
python3 -m unittest tools.rhino_oracle.test_bend
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/bend_points.json \
  --observations tools/rhino_oracle/observations/bend_points.json \
  --absolute-epsilon 1e-11 --relative-epsilon 1e-12
```

Regenerate a command capture through the private-display wrapper:

```sh
tools/rhino_oracle/run_headless.sh exec python3 - <<'PY'
import json
from pathlib import Path
from tools.rhino_oracle.client import OracleClient, load_request
request = load_request('tools/rhino_oracle/fixtures/bend_command_points.json')
response = OracleClient(settings_scheme='VibocerosOracleBend').run_rhino(request, 300)
Path('/tmp/bend-command.json').write_text(json.dumps(response))
PY
```

References: [Rhino Bend](https://docs.mcneel.com/rhino/8/help/en-us/commands/bend.htm),
[public BendSpaceMorph constructors](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/Overload_Rhino_Geometry_Morphs_BendSpaceMorph__ctor.htm).
