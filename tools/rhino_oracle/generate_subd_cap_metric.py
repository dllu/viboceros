"""Capture independent tilted and oblique annular world-metric holdouts."""
from .capture_family import capture
from .mesh_insert_inputs import save_sources
from .subd_cap_metric_cases import CASES


def request():
    return dict(protocol_version=1, iterations=1,
        tolerance=dict(absolute=1e-9, relative=1e-12, angular=1e-10),
        operations=[dict(op='subd_cap_metric', id=name, case=name) for name in CASES])


if __name__ == '__main__':
    capture('subd_cap_metric', request(), ['generate_subd_cap_metric.py',
        'subd_cap_metric_cases.py', 'subd_cap_metric_probe.py', 'subd_cap_annuli_cases.py',
        'subd_cap_regions_probe.py', 'subd_cap_regions_cases.py', 'mesh_insert_inputs.py',
        'crease_probe.py', 'crease_cases.py', 'subdivide_probe.py', 'subd_box_probe.py',
        'join_probe.py', 'merge_edges_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
        'VibocerosOracleSubDCapMetric20261029', timeout=900)
    save_sources('subd_cap_metric')
