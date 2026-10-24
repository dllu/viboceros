"""Capture fixed MeshIntersect command output from a private owned Rhino process."""
import base64
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .mesh_intersect_cases import request

HERE = Path(__file__).resolve().parent
SOURCES = ['generate_mesh_intersect.py', 'mesh_intersect_cases.py', 'mesh_intersect_probe.py',
    'mesh_boolean_command_probe.py', 'mesh_boolean_command_cases.py', 'mesh_boolean_option_cases.py',
    'mesh_boolean_sets_cases.py', 'mesh_empty_capture.py', 'mesh_boolean_cases.py',
    'join_probe.py', 'merge_edges_probe.py', 'client.py', 'rhino_worker.py', 'structural_sdk.py']


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    flags=parser.add_mutually_exclusive_group()
    flags.add_argument('--controls',action='store_true')
    flags.add_argument('--mesh-modes',action='store_true')
    flags.add_argument('--coverage',action='store_true')
    flags.add_argument('--topology',action='store_true')
    args=parser.parse_args()
    suffix = 'topology' if args.topology else 'coverage' if args.coverage else 'modes' if args.mesh_modes else 'controls' if args.controls else 'discovery'
    family='mesh_intersect_'+suffix
    output = HERE/'observations'/(family+'.json')
    if output.exists():
        raise ValueError('observations are immutable; add a family')
    payload = request(args.controls,args.mesh_modes,args.coverage,args.topology)
    query = HERE/'fixtures'/(family+'.json')
    query.write_text(json.dumps(payload, indent=2)+'\n')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {name:digest(HERE/name) for name in SOURCES}
    for name, sha in sources.items():
        path = HERE/'observations/source_snapshots'/(sha+'.gz')
        if not path.exists(): path.write_bytes(gzip.compress((HERE/name).read_bytes(), mtime=0))
    print('Capturing {} MeshIntersect workflows'.format(len(payload['operations'])), flush=True)
    scheme='VibocerosOracleMeshIntersect20261024'+suffix.capitalize()
    data = OracleClient(settings_scheme=scheme).run_rhino(payload, timeout=240)
    if sources != {name:digest(HERE/name) for name in SOURCES}:
        raise ValueError('producer changed during capture')
    data['provenance'] = dict(request_sha256=digest(query), sources=sources)
    output.write_text(json.dumps(data, indent=2, allow_nan=False)+'\n')
    folder = HERE/'fixtures'/family
    folder.mkdir(exist_ok=True)
    for row in data['results']:
        (folder/(row['id']+'.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded {} MeshIntersect workflows'.format(len(data['results'])), flush=True)


if __name__ == '__main__':
    main()
