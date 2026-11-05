"""Drive owned panel controls with actual mouse input on the private display."""
from .capture_family import capture
from .subd_edge_analysis_cases import panels_mouse, ASSETS

if __name__ == '__main__':
    request = dict(protocol_version=1, iterations=1,
                   tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=panels_mouse())
    capture('subd_edge_analysis_mouse', request,
            ['generate_subd_edge_analysis_mouse.py', 'subd_edge_analysis_cases.py',
             'subd_edge_analysis_probe.py', 'owned_panel.py', 'edge_zoom_input.py',
             'subd_direction_probe.py', 'subd_direction_cases.py', 'subd_volume_cases.py',
             'subd_area_cases.py', 'component_length_cases.py', 'subd_to_nurbs_probe.py',
             'subd_box_probe.py', 'subd_join_probe.py', 'subd_join_cases.py', 'edge_loop_cases.py',
             'join_probe.py', 'merge_edges_probe.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
             'fixtures/subd_soft_nurbs_inputs/provenance.json',
             'fixtures/subd_edge_analysis_inputs/provenance.json',
             *['fixtures/subd_soft_nurbs_inputs/' + name + '.3dm' for name in ASSETS]],
            'VibocerosOracleSubDEdgeAnalysisMouse20261105', timeout=600)
