# Direct section-fitting probes

The `sweep_refit_sections` oracle operation prepares NURBS section copies before
rail frames or surfaces are constructed. It uses the normal portable or usd-brep
section fitter. The Rhino adapter queries public `NurbsCurve.MakeCompatible`.

Requests contain `sections` (complete NURBS definitions), matching strictly
increasing finite `parameters`, and a nonnegative `refit_tolerance`. The source
limits are 256 sections, degree 16 and 512 controls per section. The measured
disabled-fitting threshold includes equality at `2 * sqrt(f64::EPSILON)`.

Each result contains the constructed inputs in `before` and `after`, fitted
definitions in `curves`, and the retained authored `parameters`. Parameter values
are metadata here; no rail placement is applied.

For example, select a coarse case from the permanent fixture:

```python
import json
from pathlib import Path
from tools.rhino_oracle.client import OracleClient

query = json.loads(Path(
    "tools/rhino_oracle/fixtures/sweep1_refit_sections.json").read_text())
query["operations"] = [op for op in query["operations"]
    if op["id"] == "closed-cubic-0.01-original-count-2"]
actual = OracleClient().run_viboceros(query)
```

Use `OracleClient(native_smlib=True)` for the usd-brep backend. Live Rhino calls
use an owned scheme and the headless runner, for example by executing a script
with `tools/rhino_oracle/run_headless.sh exec python3 my_probe.py` and calling
`OracleClient(settings_scheme="VibocerosSectionRefitExample").run_rhino(query)`.

The fixture retains fine-profile cases whose output curves still differ from
Rhino. Complete curve bounds and exact separation witnesses are checked by
`sweep_refit_make_compatible`; RPC regressions separately cover coarse and
disabled fitting, input preservation, station metadata and invalid requests.
