"""Capture immutable MeshSelfIntersect workflows with a private settings scheme."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .mesh_self_intersect_cases import request

HERE=Path(__file__).resolve().parent
SOURCES=['generate_mesh_self_intersect.py','mesh_self_intersect_cases.py',
    'mesh_self_intersect_probe.py','mesh_boolean_command_probe.py',
    'mesh_boolean_command_cases.py','mesh_boolean_option_cases.py','mesh_boolean_sets_cases.py',
    'mesh_empty_capture.py','mesh_boolean_cases.py','join_probe.py','merge_edges_probe.py',
    'client.py','rhino_worker.py','structural_sdk.py']


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--controls',action='store_true')
    args=parser.parse_args()
    suffix='controls'if args.controls else 'discovery'
    family='mesh_self_intersect_'+suffix
    output=HERE/'observations'/(family+'.json')
    if output.exists():raise ValueError('observations are immutable; add a family')
    query=HERE/'fixtures'/(family+'.json')
    query.write_text(json.dumps(request(suffix),indent=2)+'\n')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name)for name in SOURCES}
    for name,sha in sources.items():
        archive=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not archive.exists():archive.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing MeshSelfIntersect workflows',flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleMeshSelfIntersect20261024'+suffix.capitalize()).run_rhino(request(suffix),timeout=240)
    if sources!={name:digest(HERE/name)for name in SOURCES}:raise ValueError('producer changed during capture')
    data['provenance']=dict(request_sha256=digest(query),sources=sources)
    output.write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
    folder=HERE/'fixtures'/family
    folder.mkdir(exist_ok=True)
    for row in data['results']:
        (folder/(row['id']+'.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded {} MeshSelfIntersect workflows'.format(len(data['results'])),flush=True)


if __name__=='__main__':main()
