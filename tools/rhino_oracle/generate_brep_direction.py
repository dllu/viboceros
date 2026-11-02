"""Capture immutable shared-input Dir command and public face-chart observations."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .brep_direction_probe import SOURCES, EDITS

HERE = Path(__file__).resolve().parent


def request(sdk=False):
    operations=[]
    for source in SOURCES:
        for edit in EDITS:
            if sdk:
                if edit=='flip': continue
                scopes=[[0],[1,3,5],list(range(6))] if source=='box' else [[0]]
                for faces in scopes:
                    operations.append(dict(op='brep_direction_command',id='dir-sdk-'+source+'-'+edit+'-'+''.join(map(str,faces)),
                        source=source,edit=edit,preselect=True,sdk=True,faces=faces))
            elif source!='box' and (edit!='flip' or source not in ('sphere','torus')):
                for preselect in (False,True):
                    operations.append(dict(op='brep_direction_command',id='dir-'+source+'-'+edit+'-'+str(int(preselect)),
                        source=source,edit=edit,preselect=preselect))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--sdk',action='store_true')
    args=parser.parse_args();family='brep_direction_sdk' if args.sdk else 'brep_direction_command'
    query=HERE/'fixtures'/(family+'.json');output=HERE/'observations'/(family+'.json.gz')
    if query.exists() or output.exists(): raise ValueError('Dir captures and their requests are immutable')
    q=request(args.sdk);query.write_text(json.dumps(q,indent=2)+'\n')
    files=['generate_brep_direction.py','brep_direction_probe.py','client.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py']
    files+=['fixtures/brep_direction/'+name+'.3dm' for name in SOURCES]
    images={name:(HERE/name).read_bytes() for name in files}
    sources={name:hashlib.sha256(data).hexdigest() for name,data in images.items()}
    print('Capturing {} shared Dir cases'.format(len(q['operations'])),flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleDir20261102').run_rhino(q,timeout=900)
    if images!={name:(HERE/name).read_bytes() for name in files}:raise ValueError('Dir producer changed during capture')
    if len(data['results'])!=len(q['operations']) or any('error' in row or not row['value']['success'] for row in data['results']):raise ValueError('Dir capture did not finish successfully')
    if not args.sdk and any('Unknown command:' in row['value']['history'] for row in data['results']):raise ValueError('invalid Dir command macro')
    data['provenance']=dict(request_sha256=hashlib.sha256(query.read_bytes()).hexdigest(),sources=sources)
    for name,sha in sources.items():
        path=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not path.exists():path.write_bytes(gzip.compress(images[name],mtime=0))
    output.write_bytes(gzip.compress((json.dumps(data,indent=2,allow_nan=False)+'\n').encode(),mtime=0))
    print('Recorded '+str(output),flush=True)


if __name__=='__main__':main()
