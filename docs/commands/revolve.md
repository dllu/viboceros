# Revolve

Select curve profiles, then enter:

```text
Revolve axis-start axis-end sweep-degrees [StartAngle=degrees]
    [Deformable=Yes|No] [PointCount=4..1000] [DeleteInput=Yes|No]
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
retained sources stay unchanged. Its angular domain uses the profile midpoint's
distance to the **finite picked axis segment**, multiplied by the sweep radians.
The V domain is retained unless the Cash–Karp length estimate at `1e-4` exceeds
three times its span or falls below `0.33` times it. Poorly scaled domains become
`[0, estimated length]`. The geometry kernel's lower-level construction API
preserves the supplied profile chart; the command owns this native domain policy.

In the application, bare `Revolve` starts axis picking. `Angle=`, `StartAngle=`,
`Deformable=`, `PointCount=`, `DeleteInput=` and `Output=Surface` can be edited
before either pick. `FullCircle` chooses a full turn. Enter after the first pick
uses the active construction plane's Z direction. Escape cancels without changing
the model, selection or pending Redo. Invalid option groups are atomic.

The [validation record](../revolve-deformable-validation.json) covers 172 native
command workflows, 16 public-SDK diagnostic workflows and headless app replay in
both builds. Full control nets, weights, knots, domains, selection and Undo/Redo
compare within `1e-12`. Archive checks preserve active knots and geometry; 3DM may
standardize the two unused outer padding knots.

`SplitAtTangents` is accepted, but this workflow currently creates one surface.
Options reset between invocations. Arbitrary axis-contact configurations,
continuous error bounds and comparative speed remain unqualified.

```sh
cargo test -p viboceros-oracle --test revolve_deformable
cargo test -p viboceros-oracle --features native-smlib --test revolve_deformable
cargo test -p viboceros revolve_deformable
python3 -m unittest tools.rhino_oracle.test_revolve_deformable
```

All licensed captures use private Xvfb sessions through
`tools/rhino_oracle/run_headless.sh`; no desktop windows are needed.
