"""Capture Sweep1 miter edges across the crease of two unequal profiles."""
from .capture_family import capture


def request():
    rail = dict(type="polyline", vertices=[[0., 0., 0.], [5., 0., 0.], [5., 5., 0.]])
    operations = []
    for name, sign in [("inside", 1.), ("outside", -1.)]:
        first = [[0., 0., 0.], [0., sign, 1.], [0., 2. * sign, 2.5]]
        last = [[5., 5., 0.], [5. - .8 * sign, 5., 1.], [5. - 1.6 * sign, 5., 2.5]]
        operations.append(dict(
            op="sweep1", id=name, rail=rail,
            sections=[dict(type="polyline", vertices=p) for p in [first, last]],
            parameters=[0., 10.], command=True, untrimmed_miters=False,
            refit_rail=False, inspect_definition=True, inspect_edges=True,
        ))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-12, angular=1e-6),
                operations=operations)


if __name__ == "__main__":
    capture(
        "sweep1_segmented_miters", request(),
        ["generate_sweep1_segmented_miters.py", "sweep1_probe.py", "rhino_worker.py",
         "client.py", "structural_sdk.py"],
        "VibocerosOracleSweepSegmentedMiters20261120", timeout=240,
    )
