"""Capture owned SubD direction-analysis admission under private Xvfb."""
from .capture_family import capture
from .subd_direction_analysis_cases import cases

if __name__ == '__main__':
    request = dict(protocol_version=1, iterations=1,
                   tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                   operations=cases())
    capture('subd_direction_analysis_discovery', request,
            ['generate_subd_direction_analysis_discovery.py',
             'subd_direction_analysis_cases.py', 'direction_analysis_probe.py',
             'direction_analysis_input.py', 'subd_direction_probe.py',
             'subd_direction_cases.py', 'subd_volume_cases.py', 'subd_area_cases.py',
             'component_length_cases.py', 'subd_to_nurbs_probe.py', 'subd_box_probe.py',
             'subd_join_probe.py', 'subd_join_cases.py', 'edge_loop_cases.py',
             'join_probe.py', 'merge_edges_probe.py', 'structural_sdk.py',
             'rhino_worker.py', 'client.py'],
            'VibocerosOracleSubDShowDirDiscovery20261105', timeout=600)
