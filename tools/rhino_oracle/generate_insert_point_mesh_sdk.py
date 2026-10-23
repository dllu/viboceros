"""Capture independent public meshing API calibration for insertion regions."""
import gzip
import argparse
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .insert_point_mesh_sdk_cases import request
HERE = Path(__file__).resolve().parent
SOURCES = ['generate_insert_point_mesh_sdk.py', 'insert_point_mesh_sdk_cases.py',
           'insert_point_mesh_sdk_probe.py', 'client.py', 'rhino_worker.py', 'structural_sdk.py', 'join_probe.py', 'merge_edges_probe.py']


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--origins',action='store_true');args=parser.parse_args()
    family = 'insert_point_mesh_sdk_origins'if args.origins else'insert_point_mesh_sdk'
    output = HERE/'observations'/(family+'.json')
    if output.exists():raise ValueError('observations are immutable; add a family')
    query = HERE/'fixtures'/(family+'.json')
    payload = request(origins=args.origins);query.write_text(json.dumps(payload, indent=2)+'\n')
    digest = lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources = {p:digest(HERE/p) for p in SOURCES}
    for name, sha in sources.items():
        target = HERE/'observations/source_snapshots'/(sha+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/name).read_bytes(), mtime=0))
    print('Capturing '+str(len(payload['operations']))+' independent meshing recipes', flush=True)
    result = OracleClient(settings_scheme='VibocerosOracleInsertPointMeshSdk20261024'+('Origins'if args.origins else'')).run_rhino(payload, timeout=240)
    if sources != {p:digest(HERE/p) for p in SOURCES}:raise ValueError('producer changed during capture')
    result['provenance'] = dict(request_sha256=digest(query), sources=sources)
    output.write_text(json.dumps(result, indent=2, allow_nan=False)+'\n')
    print('Recorded public meshing API calibration', flush=True)


if __name__ == '__main__':main()
