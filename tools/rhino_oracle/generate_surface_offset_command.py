"""Capture fixed public surface offset command getters and undo/redo."""
import argparse,gzip,hashlib,json
from pathlib import Path
from .surface_offset_command_probe import request,through_request,select_request,validate_request
from .structural_sdk import PROBES
HERE=Path(__file__).resolve().parent
def main():
    from .client import OracleClient
    parser=argparse.ArgumentParser(description=__doc__);mode=parser.add_mutually_exclusive_group();mode.add_argument('--through-point',action='store_true');mode.add_argument('--select-point',action='store_true');parser.add_argument('--case');args=parser.parse_args()
    label='surface_offset_through_point'if args.through_point or args.select_point else'surface_offset_command'
    q=select_request()if args.select_point else through_request()if args.through_point else request()
    output='surface_offset_select_point'if args.select_point else label
    if args.case:
        q['operations']=[op for op in q['operations']if op['case']==args.case]
        if not q['operations']:parser.error('unknown fixed scene case')
        output+='_'+args.case
    validate_request(q);path=HERE/('fixtures/'+output+'.json');path.write_text(json.dumps(q,indent=2)+'\n')
    module,deps=PROBES[label];names=['generate_surface_offset_command.py','client.py','rhino_worker.py','structural_sdk.py']+[n+'.py'for n in(module,)+deps]
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources={n:digest(HERE/n)for n in names};print('Capturing',len(q['operations']),'surface offset command scenes',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleSurfaceOffsetCommand20261020').run_rhino(q,timeout=300)
    assert sources=={n:digest(HERE/n)for n in names}
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/('observations/'+output+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        p=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not p.exists():p.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded surface offset command scenes',flush=True)
if __name__=='__main__':main()
