"""Capture SoftEditSrf public getters, tensor geometry and document history."""
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


def request(pilot=False):
    folder=HERE/'fixtures/soft_edit_surface_command';folder.mkdir(exist_ok=True)
    operations=[]
    def add(name,family='surface',station=1./3.,v_fraction=2./3.,distance=None,**kw):
        operations.append(dict(op='soft_edit_surface_command',id=name,family=family,stations=[station],
            v_fraction=v_fraction,distance=distance or [4.,4.],offset=kw.pop('offset',[0.,0.,2.]),artifact_path=str(folder/(name+'.3dm')),**kw))
    if pilot:add('pilot')
    else:
        for family in('surface','surface_rational','trimmed_rectangle','trimmed_hole'):
            for u,v in[(0.,0.),(.2,.3),(1./3.,2./3.),(.5,.5),(.8,.7),(1.,1.)]:
                for distance in([1.,1.],[4.,4.],[10.,2.]):add(family+'_'+str(len(operations)),family,u,v,distance)
            for mode,kw in [('fixed',dict(fix_edges=True)),('copy',dict(copy=True)),('post',dict(preselect=False)),('cancel',dict(cancel=True)),('cancel_after',dict(cancel=True,cancel_after=True)),('normal',dict(constraint='SrfNormal')),('u_tangent',dict(constraint='UTangent')),('v_tangent',dict(constraint='VTangent')),('tilted',dict(tilted=True))]:add(family+'_'+mode,family,**kw)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=operations)
    validate_operations(q);return q


def main():
    import sys
    pilot='--pilot'in sys.argv;q=request(pilot);p=HERE/('fixtures/soft_edit_surface_pilot.json'if pilot else'fixtures/soft_edit_surface_command.json');p.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleSoftEditSurface20261019').run_rhino(q,timeout=300. if pilot else 900.)
    assert [v['id']for v in r['results']]==[o['id']for o in q['operations']]and all('error'not in o for o in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(p),sources={n:digest(HERE/n)for n in('generate_soft_edit_surface.py','soft_edit_surface_probe.py','soft_edit_curve_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    output=Path('/tmp/soft-edit-surface-pilot.json')if pilot else HERE/'observations/soft_edit_surface_command.json'
    output.write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded',len(r['results']),'SoftEditSrf workflows',flush=True)


if __name__=='__main__':main()
