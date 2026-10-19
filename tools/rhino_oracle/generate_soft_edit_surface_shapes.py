"""Capture closed seams, poles, chart changes and displacement sensitivity."""
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


def request():
    folder=HERE/'fixtures/soft_edit_surface_shapes';folder.mkdir(exist_ok=True);operations=[]
    def add(shape,label,**kw):
        name=shape+'_'+label
        operations.append(dict(op='soft_edit_surface_shapes',id=name,shape=shape,family='surface_rational',stations=[kw.pop('u',.35)],v_fraction=kw.pop('v',.6),distance=kw.pop('distance',[4.,4.]),offset=kw.pop('offset',[.3,-.2,2.]),cancel=True,cancel_after=True,artifact_path=str(folder/(name+'.3dm')),**kw))
    for shape in('flat','wave','rational','periodic','cylinder','sphere','torus'):
        for label,offset in[('small',[0.,0.,.1]),('large',[0.,0.,8.]),('negative',[-1.,.5,-2.])]:add(shape,label,offset=offset)
        for label,kw in [('fixed',dict(fix_edges=True)),('copy',dict(copy=True)),('normal',dict(constraint='SrfNormal')),('u_tangent',dict(constraint='UTangent')),('v_tangent',dict(constraint='VTangent')),('domains',dict(domains=[[-5.,17.],[2.,9.]])),('transpose',dict(transpose=True)),('reverse',dict(reverse_u=True,reverse_v=True))]:add(shape,label,**kw)
    for shape in('periodic','cylinder','sphere','torus'):
        for label,u,v in [('start',0.,0.),('end',1.,1.),('middle',.5,.5),('near_seam',.999,.3)]:add(shape,label,u=u,v=v)
    for shape in('rational','sphere','wave'):
        for label,kw in [('degree',dict(degree_u=5,degree_v=4)),('negative_gauge',dict(gauge=-8.)),('small_gauge',dict(gauge=1e-50)),('large_gauge',dict(gauge=1e50))]:add(shape,label,**kw)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=operations);validate_operations(q);return q


def main():
    q=request();p=HERE/'fixtures/soft_edit_surface_shapes.json';p.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleSoftEditSurfaceShapes20261019').run_rhino(q,timeout=900.)
    assert [v['id']for v in r['results']]==[o['id']for o in q['operations']]and all('error'not in v for v in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(p),sources={n:digest(HERE/n)for n in('generate_soft_edit_surface_shapes.py','soft_edit_surface_shapes_probe.py','soft_edit_surface_probe.py','soft_edit_curve_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (HERE/'observations/soft_edit_surface_shapes.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded',len(r['results']),'additional surface workflows',flush=True)


if __name__=='__main__':main()
