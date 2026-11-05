"""Observe the owned Edge Analysis Zoom button on hard and soft creases."""
import base64
import hashlib
import json
from .capture_family import capture, HERE
from .subd_edge_analysis_cases import panels, ASSETS

if __name__ == '__main__':
    request = dict(protocol_version=1, iterations=1,
                   tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=panels())
    capture('subd_edge_analysis_panels', request,
            ['generate_subd_edge_analysis_panels.py', 'subd_edge_analysis_cases.py',
             'subd_edge_analysis_probe.py', 'owned_panel.py', 'edge_zoom_input.py',
             'subd_direction_probe.py', 'subd_direction_cases.py', 'subd_volume_cases.py',
             'subd_area_cases.py', 'component_length_cases.py', 'subd_to_nurbs_probe.py',
             'subd_box_probe.py', 'subd_join_probe.py', 'subd_join_cases.py', 'edge_loop_cases.py',
             'join_probe.py', 'merge_edges_probe.py', 'structural_sdk.py', 'rhino_worker.py', 'client.py',
             'fixtures/subd_soft_nurbs_inputs/provenance.json',
             *['fixtures/subd_soft_nurbs_inputs/' + name + '.3dm' for name in ASSETS]],
            'VibocerosOracleSubDEdgeAnalysisPanels20261105', timeout=600)
    data = json.loads((HERE / 'observations/subd_edge_analysis_panels.json').read_bytes())
    folder = HERE / 'fixtures/subd_edge_analysis_inputs'
    manifest_path = folder / 'provenance.json'
    manifest = json.loads(manifest_path.read_bytes())
    for op, row in zip(request['operations'], data['results'], strict=True):
        scene = op['scene']
        if scene not in manifest:
            raw = base64.b64decode(row['value']['source_archive'])
            with (folder / (scene + '.3dm')).open('xb') as stream:
                stream.write(raw)
            manifest[scene] = dict(family='subd_edge_analysis_panels', case=op['id'], sha256=hashlib.sha256(raw).hexdigest())
    manifest_path.write_text(json.dumps(manifest, indent=2) + '\n')
