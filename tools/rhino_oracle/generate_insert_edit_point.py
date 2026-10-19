"""Capture InsertEditPoint geometry, input boundaries, metadata and history."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations

ROOT=Path(__file__).resolve().parents[2]


def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/insert_edit_point_command';folder.mkdir(exist_ok=True)
    operations=[]
    def add(name,family,stations=None,**kw):
        operations.append(dict(op='insert_edit_point_command',id=name,family=family,
            stations=[.35]if stations is None else stations,artifact_path=str(folder/(name+'.3dm')),**kw))
    for family in ('cubic','rational','periodic','circle','ellipse','line','polyline','polycurve'):
        for suffix,stations,kw in [('',[.35],{}),('_left',[.2],{}),('_middle',[.5],{}),('_right',[.7],{}),
            ('_start',[0.],{}),('_end',[1.],{}),('_greville_left',[1./3.],{}),('_greville_right',[2./3.],{}),
            ('_two',[.35,.7],{}),('_repeat',[.35,.35],{}),('_empty',[],{}),('_cancel',[],dict(cancel=True)),
            ('_cancel_after',[.35],dict(cancel=True)),('_post',[.35],dict(preselect=False)),
            ('_subcurve',[.35,.7],dict(preselect=False,subcurve=[.2,.8])),('_refined',[.35],dict(refine=.5))]:
            add(family+suffix,family,stations,**kw)
    for family in ('cubic','rational','periodic'):
        for suffix,kw in [('domain',dict(domain=[-3.,17.])),('reverse',dict(reverse=True)),('tilted',dict(tilted=True)),
            ('negative',dict(gauge=-8.)),('small',dict(gauge=1e-50)),('large',dict(gauge=1e50))]:
            add(family+'_'+suffix,family,**kw)
    for degree in (4,5,7):
        for family in ('cubic','rational'):
            add(family+'_degree_'+str(degree),family,[.2,.7],elevate=degree)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=operations)
    validate_operations(q)
    return q


def main():
    h=ROOT/'tools/rhino_oracle';q=request();path=h/'fixtures/insert_edit_point_command.json';path.write_text(json.dumps(q,indent=2)+'\n')
    started=time.monotonic();r=OracleClient(settings_scheme='VibocerosOracleInsertEditPointFull20261019').run_rhino(q,timeout=900.)
    assert [o['id']for o in r['results']]==[o['id']for o in q['operations']]
    assert all('error'not in o for o in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_insert_edit_point.py','insert_edit_point_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-started,request_sha256=digest(path),sources={n:digest(h/n)for n in names},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (h/'observations/insert_edit_point_command.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'InsertEditPoint workflows',flush=True)


if __name__=='__main__':main()
