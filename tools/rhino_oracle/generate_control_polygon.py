"""Capture immutable control-polygon command cases and their exact producer bytes."""
import base64
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .control_polygon_cases import request, options_request, priority_request, components_request, picks_request

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_control_polygon.py','control_polygon_cases.py','control_polygon_probe.py',
           'client.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py',
           'owned_brep_command.py','shrink_face_input.py','component_picking.py']


def main():
    from .client import OracleClient
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--options',action='store_true')
    modes.add_argument('--priority',action='store_true')
    modes.add_argument('--components',action='store_true')
    modes.add_argument('--picks',action='store_true')
    args = parser.parse_args()
    suffix = 'picks' if args.picks else 'components' if args.components else 'priority' if args.priority else 'options' if args.options else None
    family = 'control_polygon' + ('_'+suffix if suffix else '')
    query = picks_request() if args.picks else components_request() if args.components else priority_request() if args.priority else options_request() if args.options else request()
    output = HERE / 'observations' / (family+'.json')
    if output.exists():
        raise ValueError('native observations are immutable; add a new family')
    path = HERE / 'fixtures' / (family+'.json')
    path.write_text(json.dumps(query,indent=2)+'\n')
    sha = lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    hashes = {p:sha(HERE/p) for p in SOURCES}
    for name,digest in hashes.items():
        frozen = HERE / 'observations/source_snapshots' / (digest+'.gz')
        if not frozen.exists():
            frozen.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing '+str(len(query['operations']))+' control-polygon workflows',flush=True)
    response = OracleClient(settings_scheme='VibocerosOracleControlPolygon20261022'+(suffix.title() if suffix else '')).run_rhino(query,timeout=600)
    if hashes != {p:sha(HERE/p) for p in SOURCES}:
        raise ValueError('producer changed during capture')
    response['provenance'] = dict(request_sha256=sha(path),sources=hashes)
    output.write_text(json.dumps(response,indent=2,allow_nan=False)+'\n')
    for row in response['results']:
        for key,stage in [('source_3dm','source'),('result_3dm','result')]:
            if row.get('value',{}).get(key):
                target = HERE / 'fixtures' / family / stage
                target.mkdir(parents=True,exist_ok=True)
                (target/(row['id']+'.3dm')).write_bytes(base64.b64decode(row['value'][key]))
    errors = [r['id'] for r in response['results'] if 'error' in r]
    print('Recorded '+str(len(response['results']))+' rows; errors='+str(errors),flush=True)


if __name__ == '__main__':
    main()
