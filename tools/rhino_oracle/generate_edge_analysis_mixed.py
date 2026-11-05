"""Capture mixed diagnostic workflows on a private display; retain shared inputs."""
import base64
import hashlib
import json
from .capture_family import capture, HERE
from .edge_analysis_mixed_cases import cases

if __name__ == '__main__':
    query = dict(protocol_version=1, iterations=1,
                 tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=cases())
    capture('edge_analysis_mixed', query,
            ['generate_edge_analysis_mixed.py', 'edge_analysis_mixed_cases.py', 'edge_analysis_mixed_probe.py',
             'subd_edge_analysis_probe.py', 'subd_edge_analysis_cases.py', 'owned_panel.py', 'edge_zoom_input.py',
             'subd_direction_probe.py', 'subd_direction_cases.py', 'subd_volume_cases.py', 'subd_area_cases.py',
             'component_length_cases.py', 'subd_to_nurbs_probe.py', 'subd_box_probe.py', 'subd_join_probe.py',
             'subd_join_cases.py', 'edge_loop_cases.py', 'join_probe.py', 'merge_edges_probe.py',
             'structural_sdk.py', 'rhino_worker.py', 'client.py', 'fixtures/brep_direction/warped.3dm'],
            'VibocerosOracleEdgeMixed20261105', timeout=900)
    data = json.loads((HERE / 'observations/edge_analysis_mixed.json').read_bytes())
    folder = HERE / 'fixtures/edge_analysis_mixed_inputs'
    folder.mkdir(exist_ok=True)
    manifest = {}
    for op, row in zip(query['operations'], data['results'], strict=True):
        name = '_'.join(op['order'])+('_point' if op['extra_point'] else '')
        if name not in manifest:
            raw = base64.b64decode(row['value']['source_archive'])
            with (folder / (name + '.3dm')).open('xb') as stream:
                stream.write(raw)
            manifest[name] = dict(case=op['id'], sha256=hashlib.sha256(raw).hexdigest())
    (folder / 'provenance.json').write_text(json.dumps(manifest, indent=2) + '\n')
