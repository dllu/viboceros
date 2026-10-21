"""Capture fixed MeshPatch intersections without presuming native success."""
import argparse,gzip,hashlib,json
from pathlib import Path
from .mesh_patch_probe import crossing_request,crossing_verify_request,untrimmed_crossing_request,seeded_crossing_request
HERE=Path(__file__).resolve().parent
def main():
    parser=argparse.ArgumentParser(description=__doc__);group=parser.add_mutually_exclusive_group();group.add_argument('--verify',action='store_true');group.add_argument('--untrimmed',action='store_true');group.add_argument('--seeded',action='store_true');args=parser.parse_args()
    from .client import OracleClient
    name='mesh_patch_crossings_seeded'if args.seeded else'mesh_patch_crossings_untrimmed'if args.untrimmed else'mesh_patch_crossings_verify'if args.verify else'mesh_patch_crossings'
    q=seeded_crossing_request()if args.seeded else untrimmed_crossing_request()if args.untrimmed else crossing_verify_request()if args.verify else crossing_request();path=HERE/('fixtures/'+name+'.json');path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_mesh_patch_crossings.py','mesh_patch_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={name:digest(HERE/name)for name in names}
    print('Capturing MeshPatch crossings',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleMeshPatchCrossings'+('Seeded'if args.seeded else'Untrimmed'if args.untrimmed else'Verify'if args.verify else'')+'20261020').run_rhino(q,timeout=300)
    Path('/tmp/viboceros-'+name+'-last-response.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    assert sources=={name:digest(HERE/name)for name in names}
    for row in r['results']:
        if 'error'in row:raise ValueError(row)
        events=[e for e in row['value']['events']if e['name']=='MeshPatch']
        if len(events)!=1 or events[0]['result']not in ['Success','Failure']:raise ValueError('incomplete command '+row['id'])
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/('observations/'+name+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        snapshot=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not snapshot.exists():snapshot.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded MeshPatch crossings',flush=True)
if __name__=='__main__':main()
