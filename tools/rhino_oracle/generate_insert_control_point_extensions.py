"""Extension choices beyond the initial command matrix."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from .client import OracleClient
from .generate_remove_control_point import ROOT
from .structural_sdk import validate_operations


def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/insert_control_point_extensions';folder.mkdir(exist_ok=True)
    rows=[]
    def add(name,family,actions,**kw):
        rows.append(dict(op='insert_control_point_command',id=name,family=family,actions=actions,
                         artifact_path=str(folder/(name+'.3dm')),**kw))
    for family in ('surface','surface_rational','trimmed_rectangle','trimmed_hole'):
        for direction in ('U','V'):
            for side in (.2,.8):
                point=[.4,side]if direction=='U'else[side,.6]
                add(family+'_'+direction+'_'+str(int(side*10)),family,['Extend',dict(point=point)],direction=direction)
    add('repeat','surface',['Extend',dict(point=[.4,.2]),'Extend',dict(point=[.4,.8])])
    add('midpoint','surface',['Midpoint=Yes','Extend',dict(point=[.4,.8])])
    add('toggle','surface',['Extend','Toggle',dict(point=[.2,.6])])
    add('cancel_after','surface',['Extend',dict(point=[.4,.8])],cancel=True)
    for axis in ('U','V'):
        for direction in ('U','V'):
            add('periodic_'+axis+'_'+direction,'surface',['Extend',dict(point=[.2,.8])],periodic_surface_axis=axis,direction=direction)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=rows)
    if len(rows)!=24 or len({o['id']for o in rows})!=24:raise ValueError('extension case IDs must be unique')
    validate_operations(q)
    return q


def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations'
    (folder/'insert_control_point_extensions.request.json').write_text(json.dumps(q,indent=2)+'\n')
    started=time.monotonic()
    r=OracleClient(settings_scheme='VibocerosOracleInsertControlExtensions20261019').run_rhino(q,timeout=600.)
    if [x['id']for x in r['results']]!=[o['id']for o in q['operations']]:raise ValueError('incomplete extension capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_insert_control_point_extensions.py','insert_control_point_probe.py',
        'remove_knot_command_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        seconds=time.monotonic()-started,sources={n:digest(Path(__file__).with_name(n))for n in names},
        fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (folder/'insert_control_point_extensions.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'InsertControlPoint extensions',flush=True)


if __name__=='__main__':main()
