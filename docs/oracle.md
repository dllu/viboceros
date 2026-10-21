# Rhino compatibility oracle

[Documentation](README.md) · [Development and testing](development.md)

The Rust oracle and Python API run the same versioned requests against Viboceros
and the licensed Rhino installation. Captures use public APIs and command output;
saved observations support offline regression testing.

## Replay a saved comparison

From the repository root:

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/blend_lines.json \
  --observations tools/rhino_oracle/observations/blend_lines.json \
  --absolute-epsilon 5e-8
```

Replay does not launch Rhino. Use the epsilon and backend recorded by the
fixture's qualification guide; defaults are not a universal geometry tolerance.

## Capture a live comparison

On Linux, use the private Xvfb wrapper to keep captures off the shared desktop:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/core.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12
```

Run captures sequentially within a Wine prefix. Experiments that change remembered
options use a private `VibocerosOracle...` settings scheme.
[Running captures](oracle/running.md) covers dependencies, Python invocation,
backend selection and launcher behavior. [Protocol and settings](oracle/protocol.md)
covers isolation and diagnostic replay.

## Evidence and limits

The [comparison evidence index](oracle/README.md) groups retained recipes by
subsystem. Command pages remain the source for current supported behavior and
known compatibility limits; saved counts describe their captured corpus.
Sample agreement does not establish a continuous error bound. Performance
reports also require the [timing interpretation](oracle/timing.md).

<!-- Compatibility anchors for existing external links; details moved by topic. -->
<a id="blockedit-dialog-controls"></a>
<a id="directional-grip-selection"></a>
<a id="setview-option-prompt-probe"></a>
<a id="named-view-and-world-preset-policy-probes"></a>
<a id="setview-cplane-camera-probe"></a>
<a id="cplane-two-point-and-shifted-camera-probe"></a>
<a id="plan-camera-probe"></a>
<a id="world-parallel-camera-probe"></a>
<a id="world-perspective-and-two-point-camera-probe"></a>
<a id="zoom-fitting-cameras"></a>
<a id="document-clipping-and-infinite-frustum-bounds"></a>
<a id="redraw-after-navigation-and-document-edits"></a>
<a id="gpu-frustum-clipping"></a>
<a id="selection-at-the-depth-planes"></a>
<a id="snap-targets-outside-the-visible-depth-interval"></a>
<a id="camera-plane-centers-and-straight-source-representation"></a>
<a id="surface-trim-removal"></a>
<a id="interior-hole-removal-geometry-api"></a>
<a id="picked-boundary-restoration"></a>
<a id="joined-edge-separation"></a>
<a id="shrink-trimmed-surfaces"></a>
<a id="timing-interpretation"></a>
<a id="edge-analysis-topology"></a>

Detailed sections are now in the [comparison evidence index](oracle/README.md).
