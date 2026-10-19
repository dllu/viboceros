"""Capture bounded SoftEditCrv commands using public Rhino geometry and input."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations

ROOT=Path(__file__).resolve().parents[2]


def request(pilot=False):
    folder=ROOT/'tools/rhino_oracle/fixtures/soft_edit_curve_command';folder.mkdir(exist_ok=True)
    operations=[]
    def add(name,family='cubic',station=1./3.,distance=4.,**kw):
        operations.append(dict(op='soft_edit_curve_command',id=name,family=family,
            stations=[station],distance=distance,offset=kw.pop('offset',[0.,0.,2.]),artifact_path=str(folder/(name+'.3dm')),**kw))
    if pilot:
        for d in (1.,4.,10.):add('pilot_'+str(int(d)),distance=d,fix_ends=d==4.,copy=d==10.)
    else:
        for family in ('cubic','rational','periodic','circle','ellipse','line','polyline','polycurve'):
            for station in (0.,.2,1./3.,.5,.8,1.):
                for distance in (1.,10.):
                    add(family+'_'+str(len(operations)),family,station,distance)
            for label,kw in [('fixed',dict(fix_ends=True)),('copy',dict(copy=True)),('post',dict(preselect=False)),('cancel',dict(cancel=True)),('cancel_after',dict(cancel=True,cancel_after=True))]:
                add(family+'_'+label,family,**kw)
        for family in ('cubic','rational'):
            for distance in (.5,2.,5.,20.):
                for station in (.1,.5,.9):
                    add(family+'_dense_'+str(len(operations)),family,station,distance,dense=True)
        for family in ('cubic','rational','periodic'):
            for label,kw in [('domain',dict(domain=[-3.,17.])),('reverse',dict(reverse=True)),('tilted',dict(tilted=True)),('refined',dict(refine=.5)),('negative',dict(gauge=-8.)),('small',dict(gauge=1e-50)),('large',dict(gauge=1e50)),('zero',dict(offset=[0.,0.,0.],cancel=True,cancel_after=True)),('fixed_long',dict(fix_ends=True,distance=20.))]:
                add(family+'_'+label,family,**kw)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=operations)
    validate_operations(q);return q


def main():
    import sys
    pilot='--pilot'in sys.argv
    h=ROOT/'tools/rhino_oracle';q=request(pilot);path=h/('fixtures/soft_edit_curve_pilot.json'if pilot else'fixtures/soft_edit_curve_command.json')
    path.write_text(json.dumps(q,indent=2)+'\n')
    started=time.monotonic();r=OracleClient(settings_scheme='VibocerosOracleSoftEditCurve20261019').run_rhino(q,timeout=900.)
    assert [o['id']for o in r['results']]==[o['id']for o in q['operations']]
    assert all('error'not in o for o in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_soft_edit_curve.py','soft_edit_curve_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-started,request_sha256=digest(path),sources={n:digest(h/n)for n in names},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    output=Path('/tmp/soft-edit-curve-pilot.json')if pilot else h/'observations/soft_edit_curve_command.json'
    output.write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'SoftEditCrv workflows',flush=True)


if __name__=='__main__':main()
