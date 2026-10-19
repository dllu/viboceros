"""Capture InsertKink from independent source files, including curve/surface getters."""
from pathlib import Path
import hashlib,json,subprocess,time
from .client import OracleClient
from .structural_sdk import validate_operations
ROOT=Path(__file__).resolve().parents[2]
def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/insert_kink';folder.mkdir(exist_ok=True)
    def op(name,family,stations=None,**kw):return dict(op='insert_kink_command',id=name,family=family,stations=[.35]if stations is None else stations,artifact_path=str(folder/(name+'.3dm')),**kw)
    out=[]
    for family in('cubic','circle','line','polyline','ellipse','polycurve','rational','periodic'):
        out.extend([op(family,family),op(family+'_two',family,[.35,.7]),op(family+'_repeat',family,[.35,.35]),
                    op(family+'_ends',family,[0.,1.]),op(family+'_cancel',family,cancel=True),op(family+'_post',family,preselect=False)])
    out.extend([op('polyline_vertex','polyline',[1./3.]),op('polycurve_joint','polycurve',[3./(3.+2.*3.141592653589793)]),
                op('refined_cubic','cubic',[.5],refine=.5),op('elevated','cubic',elevate=5),op('elevated_rational','rational',elevate=7),
                op('reversed','rational',reverse=True),op('tilted','cubic',tilted=True),op('domain','rational',domain=[-3.,17.]),
                op('circle_domain','circle',domain=[-3.,17.]),op('negative_gauge','rational',gauge=-8.),
                op('near_start','cubic',[1e-12]),op('near_start_line','line',[1e-12]),op('numeric_invalid','cubic',numeric=True)])
    for family in('surface','surface_rational'):
        for direction in('U','V','Both'):
            for symmetrical in(False,True):out.append(op(family+'_'+direction+('_sym'if symmetrical else''),family,direction=direction,symmetrical=symmetrical))
        out.extend([op(family+'_two',family,[.35,.7],direction='Both'),op(family+'_repeat',family,[.35,.35]),
                    op(family+'_cancel',family,cancel=True),op(family+'_empty',family,[]),op(family+'_midpoints',family,midpoints=True),
                    op(family+'_post',family,preselect=False,direction='V'),op(family+'_ends',family,[0.,1.],direction='V')])
    for family in('trimmed_rectangle','trimmed_hole'):
        out.append(op(family,family,v_fraction=.8,direction='Both',symmetrical=True))
        out.append(op(family+'_cancel',family,v_fraction=.8,cancel=True))
    for family in('cubic','circle','rational'):
        out.extend([op(family+'_subcurve',family,[.35,.7],subcurve=[.2,.8],preselect=False),
                    op(family+'_subcurve_empty',family,[],subcurve=[.2,.8],preselect=False),
                    op(family+'_subcurve_cancel',family,subcurve=[.2,.8],preselect=False,cancel=True)])
    out.append(op('polysurface_rejected','box',face=2,direction='Both'))
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=out);validate_operations(q);return q

def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations';(folder/'insert_kink.request.json').write_text(json.dumps(q,indent=2)+'\n')
    start=time.monotonic();r=OracleClient(settings_scheme='VibocerosOracleInsertKink20261019').run_rhino(q,timeout=420.)
    if [v['id']for v in r['results']]!=[v['id']for v in q['operations']]:raise ValueError('incomplete InsertKink capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();names=('insert_kink_probe.py','generate_insert_kink.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-start,
        sources={n:digest(Path(__file__).with_name(n))for n in names},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (folder/'insert_kink.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded',len(r['results']),'actual InsertKink commands',flush=True)
if __name__=='__main__':main()
