# Revolve

Select curve profiles, then enter:

```text
Revolve axis-start axis-end sweep-degrees [StartAngle=degrees]
    [Deformable=Yes|No] [PointCount=4..1000] [DeleteInput=Yes|No]
    [SplitAtTangents=Yes|No]
Revolve axis-start axis-end FullCircle
```

Sweeps are signed, nonzero and at most 360 degrees. Defaults are a zero start
angle, exact output, ten deformable points and retained inputs. `Output=Surface`
is accepted; SubD output is unsupported. Exact output uses rational quadratic
quadrant spans in U and the profile in V.

`Deformable=Yes` creates cubic angular spans with simple interior knots. Partial
turns have the requested number of U controls and tangent-aligned end handles.
Full turns use a periodic basis with three repeated controls beyond that count.
The profile's degree and rational weights remain in V. The angular fit is solved
once in a unit frame and mapped into every profile row.

The command clamps active profile ends and keeps the leading portion up to its
first interior contact with the infinite axis. This affects only the output;
retained sources stay unchanged. Deformable output's angular domain uses the
profile midpoint's distance to the **finite picked axis segment**, multiplied by
the sweep radians. Exact output uses the 65-sample maximum-radius estimate
around the infinite axis. The V domain is retained unless the Cash–Karp length estimate at `1e-4` exceeds
three times its span or falls below `0.33` times it. Poorly scaled domains become
`[0, estimated length]`. If an interior span is too narrow for integration,
exact Bézier spans are integrated in individual unit charts.
The geometry kernel's lower-level construction API
preserves the supplied profile chart; the command owns this native domain policy.

In the application, bare `Revolve` starts axis picking. `Angle=`, `StartAngle=`,
`Deformable=`, `PointCount=`, `SplitAtTangents=`, `DeleteInput=` and
`Output=Surface` can be edited before either pick. `FullCircle` chooses a full turn. Enter after the first pick
uses the active construction plane's Z direction. Escape cancels without changing
the model, selection or pending Redo. Invalid option groups are atomic.

The [validation record](../revolve-deformable-validation.json) covers 172 native
command workflows, 16 public-SDK diagnostic workflows and headless app replay in
both builds. Full control nets, weights, knots, domains, selection and Undo/Redo
compare within `1e-12`. Archive checks preserve active knots and geometry; 3DM may
standardize the two unused outer padding knots.

`SplitAtTangents=Yes` splits the common surface at G2 breaks using a one-degree
angular threshold. Global `CreaseSplitting Enable` independently splits G1
breaks at 0.1 degrees; it defaults to enabled. Both apply to exact and deformable
output. G2 splitting runs first, then crease splitting preserves existing face
slots. Smooth redundant knots stay unsplit; shared boundaries, seams and pole
trims remain sewn. Every face retains the common angular domain.

The [splitting validation record](../revolve-splitting-validation.json) adds 160
native workflows, interactive replay and 3DM/meshing checks in both builds.
Controls, weights, active knots, domains and boundary samples compare within
`1e-12`; face ordering and edge incidence also match. Native edge numbering and
analytic parameter clocks are not reproduced. Options reset between invocations.
Arbitrary axis-contact configurations, continuous error bounds and comparative
speed remain unqualified.

```sh
cargo test -p viboceros-oracle --test revolve_deformable --test revolve_splitting
cargo test -p viboceros-oracle --features native-smlib --test revolve_deformable --test revolve_splitting
cargo test -p viboceros revolve_
python3 -m unittest tools.rhino_oracle.test_revolve_splitting tools.rhino_oracle.test_revolve_deformable
```

All licensed captures use private Xvfb sessions through
`tools/rhino_oracle/run_headless.sh`; no desktop windows are needed.
