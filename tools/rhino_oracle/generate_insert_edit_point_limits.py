"""Capture coincidence thresholds, periodic padding and picker cancellation."""
import hashlib,json,subprocess
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations


def request():
    h=Path(__file__).resolve().parent;folder=h/'fixtures/insert_edit_point_limits';folder.mkdir(exist_ok=True)
    ops=[]
    def add(name,family,stations,**kw):
        ops.append(dict(op='insert_edit_point_limits',id=name,family=family,stations=stations,artifact_path=str(folder/(name+'.3dm')),**kw))
    for scale in(1e-3,1.,1e6):
        for exponent in(4,6,8,10,12):
            for sign in(-1.,1.):
                add('near_'+str(len(ops)),'cubic',[1./3.+sign*10.**-exponent],scale=scale)
    for fraction in(.01,.05,.1,.9,.95,.99):add('periodic_'+str(len(ops)),'periodic',[fraction])
    for family in('cubic','circle','periodic','polycurve'):
        add(family+'_post_cancel',family,[.35],preselect=False,cancel=True)
        add(family+'_post_empty',family,[],preselect=False)
        add(family+'_subcurve_cancel',family,[.35],subcurve=[.2,.8],preselect=False,cancel=True)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q


def main():
    h=Path(__file__).resolve().parent;q=request();p=h/'fixtures/insert_edit_point_limits.json';p.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleInsertEditPointLimits20261019').run_rhino(q,timeout=600.)
    assert [o['id']for o in r['results']]==[o['id']for o in q['operations']]
    assert all('error'not in o for o in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_insert_edit_point_limits.py','insert_edit_point_limits_probe.py','insert_edit_point_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(p),sources={n:digest(h/n)for n in names},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (h/'observations/insert_edit_point_limits.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'InsertEditPoint limit workflows',flush=True)


if __name__=='__main__':main()
