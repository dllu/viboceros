# Oracle protocol and private settings

[Oracle overview](../oracle.md) · [Evidence index](README.md)

The versioned Python oracle API runs identical JSON geometry and document-state
batches in a native release build of Viboceros and Rhino 8, recursively checks
results, and reports per-operation timings.
Both ordinary engine runners require the returned recipe ID set to exactly match
the request. Missing or unexpected outcomes fail protocol validation before a
capture can be accepted; result order may differ. The retained initial singular
Rebuild artifact has ten results for twelve requested recipes and fails this check.
Its fresh [verified capture](../../tools/rhino_oracle/observations/surface_rebuild_singular_trim.json)
contains all twelve outcomes.

Standard geometry/command batches apply the request's absolute, relative, and angular tolerances to
Rhino's active document and restores its previous settings on success or failure.
This matters for command macros, which read document settings rather than an API
tolerance argument. See [Rhino's document tolerance API](https://developer.rhino3d.com/api/rhinocommon/rhino.rhinodoc/modelabsolutetolerance).
Older command comparisons made before this synchronization need revalidation.

[Diagnostic replay and Python API](../oracle-replay.md) retain per-operation native
errors while comparing every successful record against saved Rhino observations.
On Linux, the CLI automatically runs live `rhino` and `compare` probes under
the dedicated `run_headless.sh` Xvfb display. Direct Python API calls to
`OracleClient.run_rhino` on Linux require an Xvfb session established by this
wrapper. Replay and native-only modes do not launch Rhino.

For experiments that change application preferences or remembered command
defaults, use a private settings scheme:

```python
client = OracleClient(settings_scheme="VibocerosOracleCopy_20261002")
observed = client.run_rhino(request)
```

Run this Python code through `tools/rhino_oracle/run_headless.sh exec` as usual.
Scheme names must start with `VibocerosOracle`, followed by 1–64 ASCII letters,
digits, underscores, or hyphens. The client passes a single `/scheme=` launcher
argument. Rhino stores the scheme's options separately, as documented in
[McNeel's startup options](https://docs.mcneel.com/rhino/8/help/en-us/information/startingrhino.htm).
The [50-step Copy preference workflow](../commands/remember-copy-options.md#native-verification)
uses this launch path to measure command starts, completed choices, cancellation,
and re-enabling in Rhino 8.32. Copy settings probes require a private scheme.
Run native captures sequentially within a Wine prefix.
