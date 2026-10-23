"""Capture immutable native face-threshold inputs and exact source archives."""
import base64
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .nonplanar_sample_cases import request
HERE=Path(__file__).resolve().parent
SOURCES=['generate_nonplanar_sample.py','nonplanar_sample_cases.py','nonplanar_sample_probe.py',
    'crease_probe.py','crease_cases.py','subdivide_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py',
    'client.py','rhino_worker.py','structural_sdk.py','conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','catenary_point_picker.py','group_picking.py']


FAMILIES={
    'discovery':('nonplanar_sample_discovery','Discovery',300),
    'shaded':('nonplanar_sample_shaded','Shaded',300),
    'main':('nonplanar_sample_main_v2','MainV2',900),
    'core':('nonplanar_sample_core','Core',600),
    'rules':('nonplanar_sample_rules','Rules',300),
    'external':('nonplanar_sample_external','External',120),
    'raw':('nonplanar_sample_raw','Raw',120),
    'quad_ngon':('nonplanar_sample_quad_ngon','QuadNgon',120),
    'neighbors':('nonplanar_sample_neighbors','Neighbors',180),
}

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.set_defaults(family='discovery')
    flags=parser.add_mutually_exclusive_group()
    for flag in FAMILIES:
        if flag!='discovery':flags.add_argument('--'+flag.replace('_','-'),dest='family',action='store_const',const=flag)
    args=parser.parse_args();family,suffix,timeout=FAMILIES[args.family]
    output=HERE/'observations'/(family+'.json')
    if output.exists():raise ValueError('native observations are immutable; add a family')
    query=HERE/'fixtures'/(family+'.json');payload=request(**({args.family:True}if args.family!='discovery'else{}));query.write_text(json.dumps(payload,indent=2)+'\n')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={p:digest(HERE/p)for p in SOURCES}
    for name,sha in sources.items():
        target=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not target.exists():target.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Capturing physical face threshold getters',flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleNonplanarFaces20261023'+suffix).run_rhino(payload,timeout=timeout)
    if sources!={p:digest(HERE/p)for p in SOURCES}:raise ValueError('producer changed during capture')
    data['provenance']=dict(request_sha256=digest(query),sources=sources)
    output.write_text(json.dumps(data,indent=2,allow_nan=False)+'\n')
    folder=HERE/'fixtures'/family;folder.mkdir(exist_ok=True)
    for row in data['results']:
        if row.get('value',{}).get('source_3dm'):
            (folder/(row['id']+'.3dm')).write_bytes(base64.b64decode(row['value']['source_3dm']))
    print('Recorded '+str(len(data['results']))+' native getters',flush=True)
if __name__=='__main__':main()
