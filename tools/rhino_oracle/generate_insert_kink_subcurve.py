"""Independent captures for temporary curve length and direction input."""
from pathlib import Path
import hashlib,json,subprocess,time
from .client import OracleClient
from .structural_sdk import validate_operations
ROOT=Path(__file__).resolve().parents[2]
def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/insert_kink_subcurve';folder.mkdir(exist_ok=True)
    def op(name,family,getter,stations=None,**kw):return dict(op='insert_kink_subcurve',id=name,family=family,getter=getter,stations=[getter['start']]if stations is None else stations,artifact_path=str(folder/(name+'.3dm')),**kw)
    rows=[]
    for family in('line','cubic','rational','circle','ellipse','polyline','polycurve','periodic'):
        rows.extend([
            op(family+'_forward',family,dict(start=.2,lengths=[1.],confirm=.8)),
            op(family+'_backward',family,dict(start=.8,lengths=[1.],confirm=.2)),
            op(family+'_negative',family,dict(start=.5,lengths=[-1.],confirm=.8)),
            op(family+'_back_points',family,dict(start=.8,end=.2)),
            op(family+'_zero',family,dict(start=.2,lengths=[0.]),stations=[]),
            op(family+'_whole',family,dict(start=.2,lengths=['Whole'],confirm=.8)),
            op(family+'_repair',family,dict(start=.2,lengths=[1000.,2.,1.],confirm=.8)),
            op(family+'_no_confirm',family,dict(start=.2,lengths=[1.]),stations=[]),
            op(family+'_cancel_getter',family,dict(start=.2,lengths=[1.],cancel_getter=True),stations=[]),
            op(family+'_cancel_kink',family,dict(start=.2,lengths=[1.],confirm=.8),cancel=True),
        ])
        rows.extend([
            op(family+'_lock_numeric',family,dict(start=.2,lock_hover=.4,lengths=[1.],confirm=.8)),
            op(family+'_lock_back_numeric',family,dict(start=.8,lock_hover=.6,lengths=[1.],confirm=.2)),
            op(family+'_lock_cross_numeric',family,dict(start=.2,lock_hover=.4,lengths=[8.],confirm=.8)),
            op(family+'_lock_wrong_point',family,dict(start=.5,lock_hover=.8,end=.2)),
            op(family+'_unlock_point',family,dict(start=.5,lock_hover=.8,unlock=True,end=.2)),
            op(family+'_toggle_unlock',family,dict(start=.5,lock_hover=.8,toggle_unlock=True,end=.2)),
        ])
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=rows);validate_operations(q);return q

def capture(q,destination,scheme):
    start=time.monotonic();r=OracleClient(settings_scheme=scheme).run_rhino(q,timeout=420.)
    assert [v['id']for v in r['results']]==[v['id']for v in q['operations']]
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('insert_kink_subcurve_probe.py','generate_insert_kink_subcurve.py','insert_kink_probe.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py','shrink_face_input.py','snap_environment.py','insert_kink_subcurve_input.py','subcurve_direction_input.py','group_picking.py','client.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-start,sources={n:digest(Path(__file__).with_name(n))for n in names},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    Path(destination).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded',len(r['results']),'temporary InsertKink getters',flush=True)
def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations';(folder/'insert_kink_subcurve.request.json').write_text(json.dumps(q,indent=2)+'\n')
    capture(q,folder/'insert_kink_subcurve.json','VibocerosOracleKinkSubcurve20261019')
if __name__=='__main__':main()
