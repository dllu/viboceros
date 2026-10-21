"""Capture fixed public MeshFromLines SDK and command observations."""
import argparse,gzip,hashlib,json
from pathlib import Path
from .mesh_from_lines_probe import request,variant_request,control_request,edge_request
HERE=Path(__file__).resolve().parent

def main():
    from .client import OracleClient
    parser=argparse.ArgumentParser(description=__doc__);group=parser.add_mutually_exclusive_group();group.add_argument('--variants',action='store_true');group.add_argument('--controls',action='store_true');group.add_argument('--edges',action='store_true');args=parser.parse_args();name='mesh_from_lines_edges'if args.edges else'mesh_from_lines_controls'if args.controls else'mesh_from_lines_variants'if args.variants else'mesh_from_lines'
    q=edge_request()if args.edges else control_request()if args.controls else variant_request()if args.variants else request();path=HERE/('fixtures/'+name+'.json');path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_mesh_from_lines.py','mesh_from_lines_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name)for name in names}
    print('Capturing MeshFromLines',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleMeshFromLines'+('Edges'if args.edges else'Controls'if args.controls else'Variants'if args.variants else'')+'20261020').run_rhino(q,timeout=300)
    Path('/tmp/viboceros-mesh-from-lines-last-response.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    assert sources=={name:digest(HERE/name)for name in names}
    for row in r['results']:
        if 'error'in row:raise ValueError(row)
        events=[e for e in row['value']['events']if e['name']=='MeshFromLines']
        if len(events)!=1:raise ValueError('incomplete command '+row['id'])
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/('observations/'+name+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded MeshFromLines',flush=True)
if __name__=='__main__':main()
