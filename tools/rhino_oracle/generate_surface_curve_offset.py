"""Capture public OffsetOnSurface geometry independently of the native backend."""
import argparse,gzip,hashlib,json,re
from pathlib import Path
from .surface_curve_offset_probe import request,clipping_request,validate_request
HERE=Path(__file__).resolve().parent
def main():
    from .client import OracleClient
    parser=argparse.ArgumentParser(description=__doc__);group=parser.add_mutually_exclusive_group()
    group.add_argument('--verify',action='store_true');group.add_argument('--candidate-report',type=Path);group.add_argument('--clipping',action='store_true')
    args=parser.parse_args();label='surface_curve_offset_clipping'if args.clipping else'surface_curve_offset_verify'if args.verify or args.candidate_report else'surface_curve_offset'
    path=HERE/('fixtures/'+label+'.json')
    if args.verify:q=json.loads(path.read_text())
    elif args.candidate_report:
        rows={r['id']:r for r in [json.loads(s)for s in re.findall(r'SURFACE_GEODESIC (\{[^\n]+\})',args.candidate_report.read_text())]}
        cases={'plane_curve_0','plane_curve_2','warped_iso_0','warped_iso_1','warped_iso_2','cylinder_axis_2'}
        q=request();q['operations']=[dict(op,id=op['id']+'_verify',samples=[p['point']for p in rows[op['id']]['candidates']])for op in q['operations']if op['id']in cases]
    elif args.clipping:q=clipping_request()
    else:q=request()
    validate_request(q);path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_surface_curve_offset.py','surface_curve_offset_probe.py','client.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={n:digest(HERE/n)for n in names}
    print('Capturing',len(q['operations']),'surface offset SDK cases',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleSurfaceOffset20261020').run_rhino(q,timeout=300)
    assert sources=={n:digest(HERE/n)for n in names}
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/('observations/'+label+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        p=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not p.exists():p.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded surface offset SDK cases',flush=True)
if __name__=='__main__':main()
