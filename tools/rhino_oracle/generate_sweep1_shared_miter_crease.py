"""Capture open profiles whose lateral offsets cancel at a rail corner."""
from .capture_family import capture


def request():
    rail = dict(type="polyline", vertices=[[0., 0., 0.], [5., 0., 0.], [5., 5., 0.]])
    operations = []
    for name, offset in [("sloping", 2.), ("vertical", 1.)]:
        sections = [
            dict(type="polyline", vertices=[[0., 0., 0.], [0., -1., 1.], [0., -offset, 2.5]]),
            dict(type="polyline", vertices=[[5., 5., 0.], [4., 5., 1.], [5. - offset, 5., 2.5]]),
        ]
        for untrimmed in [False, True]:
            mode = "untrimmed" if untrimmed else "trimmed"
            operations.append(dict(
                op="sweep1", id=name + "-" + mode, rail=rail, sections=sections,
                parameters=[0., 10.], command=True, untrimmed_miters=untrimmed,
                refit_rail=False, inspect_definition=True,
            ))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-7, relative=1e-12, angular=1e-6),
                operations=operations)


if __name__ == "__main__":
    capture(
        "sweep1_shared_miter_crease", request(),
        ["generate_sweep1_shared_miter_crease.py", "sweep1_probe.py", "rhino_worker.py",
         "client.py", "structural_sdk.py"],
        "VibocerosOracleSweepSharedMiterCrease20261120", timeout=240,
    )
