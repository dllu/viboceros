"""Generate native SimplifyCrv command observations and exact source 3dm files."""
from pathlib import Path
import hashlib,json,subprocess,time
from .client import OracleClient
from .simplify_curve_probe import FAMILIES
from .structural_sdk import validate_operations
ROOT=Path(__file__).resolve().parents[2]
def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/simplify_curves';folder.mkdir(exist_ok=True)
    def op(name,family=None,**extra):return dict(op='simplify_curve_command',id=name,family=family or name,artifact_path=str(folder/(name+'.3dm')),**extra)
    ops=[op(name)for name in FAMILIES]
    for name,domain in [('nurbs_line',[-2.,8.]),('nurbs_arc',[-3.,7.]),('polyline_collinear',[-5.,15.]),('polycurve_arcs',[20.,40.])]:ops.append(op(name+'_domain',name,domain=domain))
    for delta,label in [(1e-5,'outside'),(1e-6,'at'),(5e-7,'inside')]:ops.append(op('near_line_'+label,'near_line',delta=delta))
    ops.append(op('mixed_selection','nurbs_line',mixed=True))
    for delta,label in [(1e-6,'small'),(2e-6,'threshold'),(1e-5,'wide')]:ops.append(op('near_tangent_'+label,'near_tangent',delta=delta))
    for family in ('nurbs_line','mixed_arc_run','cubic','polyline_collinear'):
        ops.append(op(family+'_post',family,preselect=False))
    ops.append(op('batch_simplification','mixed_arc_run',copies=3))
    ops.append(op('batch_post','polyline_collinear',copies=3,preselect=False))
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q

def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations';started=time.monotonic()
    (folder/'simplify_curves.request.json').write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleSimplifyCurves20261018FinalV2').run_rhino(q,timeout=300.)
    if len(r['results'])!=len(q['operations'])or not all(row['value']['success']for row in r['results']):raise ValueError('incomplete SimplifyCrv capture')
    files=['simplify_curve_probe.py','generate_simplify_curves.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-started,
        sources={name:digest(Path(__file__).with_name(name))for name in files},fixtures={Path(op['artifact_path']).name:digest(Path(op['artifact_path']))for op in q['operations']})
    (folder/'simplify_curves.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'actual SimplifyCrv commands',flush=True)
if __name__=='__main__':main()
