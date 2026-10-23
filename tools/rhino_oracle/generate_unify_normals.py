"""Capture immutable public normal-unification references and source archives."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .unify_normals_cases import CASE_GROUPS,request
HERE=Path(__file__).resolve().parent
SOURCES=['generate_unify_normals.py','unify_normals_cases.py','unify_normals_seed_cases.py','unify_normals_walk_cases.py','unify_normals_probe.py','mesh_channel_capture.py','crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py']


def main():
    parser=argparse.ArgumentParser(description=__doc__);flags=parser.add_mutually_exclusive_group()
    for flag in CASE_GROUPS:
        if flag!='sdk':flags.add_argument('--'+flag,action='store_true')
    args=parser.parse_args();suffix=next((flag for flag,enabled in vars(args).items()if enabled),'sdk')
    family='unify_normals_'+suffix;output=HERE/'observations'/(family+'.json')
    if output.exists():raise ValueError('native observations are immutable; add a family')
    query=HERE/'fixtures'/(family+'.json');payload=request(**vars(args));query.write_text(json.dumps(payload,indent=2)+'\n')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:digest(HERE/p)for p in SOURCES}
    for name,sha in sources.items():
        target=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing '+str(len(payload['operations']))+' native normal unification workflows',flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleUnifyNormals20261023'+suffix.title()).run_rhino(payload,timeout=240)
    if sources!={p:digest(HERE/p)for p in SOURCES}:raise ValueError('producer changed during capture')
    data['provenance']=dict(request_sha256=digest(query),sources=sources);output.write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
    for row in data['results']:
        if row.get('value',{}).get('source_3dm'):
            folder=HERE/'fixtures'/family;folder.mkdir(exist_ok=True)
            (folder/(row['id']+'.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded '+str(len(data['results']))+' native workflows',flush=True)
if __name__=='__main__':main()
