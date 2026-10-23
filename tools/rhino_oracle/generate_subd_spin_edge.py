"""Capture immutable SubD edge spins, repeat steps and local Undo."""
import base64
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .subd_spin_edge_cases import request
HERE=Path(__file__).resolve().parent
SOURCES=['generate_subd_spin_edge.py','subd_spin_edge_cases.py','subd_spin_edge_probe.py',
         'catenary_point_picker.py','group_picking.py','subd_edit_scene.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','number_token.py','crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py',
         'join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py']


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    choices=parser.add_mutually_exclusive_group()
    families={'discovery':'Discovery','live':'Live','live_v2':'LiveV2','live_v3':'LiveV3','holdouts':'Holdouts','live_v4':'LiveV4','live_v5':'LiveV5'}
    for flag in families:choices.add_argument('--'+flag.replace('_','-'),action='store_true')
    args=parser.parse_args();flag=next((key for key in families if getattr(args,key)),None)
    family='subd_spin_edge'+('_'+flag if flag else'');query=HERE/'fixtures'/(family+'.json')
    output=HERE/'observations'/(family+'.json')
    if output.exists():raise ValueError('native observations are immutable; add a family')
    payload=request(**({flag:True}if flag else{}))
    query.write_text(json.dumps(payload,indent=2)+'\n')
    sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:sha(HERE/p)for p in SOURCES}
    for name,digest in sources.items():
        target=HERE/'observations/source_snapshots'/(digest+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing '+str(len(payload['operations']))+' SubD edge-spin scenes',flush=True)
    result=OracleClient(settings_scheme='VibocerosOracleSubDSpinEdge20261023'+(families[flag]if flag else'Main')).run_rhino(payload,timeout=480)
    if sources!={p:sha(HERE/p)for p in SOURCES}:raise ValueError('producer changed during capture')
    result['provenance']=dict(request_sha256=sha(query),sources=sources)
    output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    folder=HERE/'fixtures'/family;folder.mkdir(exist_ok=True)
    for row in result['results']:
        if row.get('value',{}).get('source_3dm'):
            (folder/(row['id']+'.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded '+str(len(result['results']))+' scenes',flush=True)


if __name__=='__main__':main()
