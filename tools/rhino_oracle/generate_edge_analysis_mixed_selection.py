"""Capture mixed diagnostic workflows on a private display; retain shared inputs."""
import base64
import hashlib
import json
from .capture_family import capture, HERE
from .edge_analysis_mixed_cases import selection_cases

if __name__ == '__main__':
    query = dict(protocol_version=1, iterations=1,
                 tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=selection_cases())
    capture('edge_analysis_mixed_selection', query,
            ['generate_edge_analysis_mixed_selection.py', 'edge_analysis_mixed_cases.py', 'edge_analysis_mixed_probe.py',
             'subd_edge_analysis_probe.py', 'subd_edge_analysis_cases.py', 'owned_panel.py', 'edge_zoom_input.py',
             'subd_direction_probe.py', 'subd_direction_cases.py', 'subd_volume_cases.py', 'subd_area_cases.py',
             'component_length_cases.py', 'subd_to_nurbs_probe.py', 'subd_box_probe.py', 'subd_join_probe.py',
             'subd_join_cases.py', 'edge_loop_cases.py', 'join_probe.py', 'merge_edges_probe.py',
             'structural_sdk.py', 'rhino_worker.py', 'client.py', 'fixtures/brep_direction/warped.3dm'],
            'VibocerosOracleEdgeMixedSelection20261105', timeout=900)
