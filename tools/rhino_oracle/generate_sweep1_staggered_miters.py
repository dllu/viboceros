"""Capture trimmed Sweep1 miters with profile creases at different heights."""
from copy import deepcopy

from .capture_family import capture
from .generate_sweep1_segmented_miters import request as aligned_request


def request():
    root = aligned_request()
    operations = []
    for original in root["operations"]:
        operation = deepcopy(original)
        operation["sections"][1]["vertices"][1][2] = 1.35
        operations.append(operation)
    root["operations"] = operations
    return root


if __name__ == "__main__":
    capture(
        "sweep1_staggered_miters", request(),
        ["generate_sweep1_staggered_miters.py", "generate_sweep1_segmented_miters.py",
         "sweep1_probe.py", "rhino_worker.py", "client.py", "structural_sdk.py"],
        "VibocerosOracleSweepStaggeredMiters20261120", timeout=240,
    )
