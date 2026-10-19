"""Record public closest-point parameters near an insertion decision boundary."""
import hashlib,json,subprocess
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations


def main():
    h=Path(__file__).resolve().parent;folder=h/'fixtures/insert_edit_point_projection';folder.mkdir(exist_ok=True)
    ops=[]
    for scale in(1e-3,1.,1e6):
        for delta in(-1e-8,0.,1e-8):
            name='projection_'+str(len(ops));ops.append(dict(op='insert_edit_point_limits',id=name,family='cubic',scale=scale,stations=[1./3.+delta],artifact_path=str(folder/(name+'.3dm'))))
    for delta in(-1e-6,-1e-8,-1e-10,0.,1e-10,1e-8,1e-6):
        name='periodic_'+str(len(ops));ops.append(dict(op='insert_edit_point_limits',id=name,family='periodic',stations=[.25+delta],artifact_path=str(folder/(name+'.3dm'))))
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q)
    p=h/'fixtures/insert_edit_point_projection.json';p.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleInsertEditPointProjection20261019').run_rhino(q,timeout=600.)
    assert [o['id']for o in r['results']]==[o['id']for o in q['operations']]
    assert all('error'not in o for o in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_insert_edit_point_projection.py','insert_edit_point_limits_probe.py','insert_edit_point_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(p),sources={n:digest(h/n)for n in names},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in ops})
    (h/'observations/insert_edit_point_projection.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'projection workflows',flush=True)


if __name__=='__main__':main()
