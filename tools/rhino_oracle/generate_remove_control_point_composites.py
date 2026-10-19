"""Composite endpoints, shared junctions, parent charts and transformed grips."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from .client import OracleClient
from .generate_remove_control_point import ROOT
from .structural_sdk import validate_operations


def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/remove_control_point_composites';folder.mkdir(exist_ok=True)
    rows=[]
    def add(name, point=None, **kw):
        actions=kw.pop('actions', [] if point is None else [dict(point=point)])
        rows.append(dict(op='remove_control_point_command',id=name,family='polycurve',
                         actions=actions,seed=False,artifact_path=str(folder/(name+'.3dm')),**kw))
    add('reverse_tail_line',[0.,0.,0.],reverse=True)
    add('reverse_shared_joint',[3.,0.,0.],reverse=True)
    add('reverse_arc_inner_low',[5.,0.,0.],reverse=True)
    add('reverse_arc_inner_high',[5.,4.,0.],reverse=True)
    add('forward_arc_end',[3.,4.,0.])
    add('reverse_arc_start',[3.,4.,0.],reverse=True)
    add('forward_line_joint',[3.,0.,0.])
    add('forward_empty')
    add('reverse_empty',reverse=True)
    add('reverse_cancel',reverse=True,cancel=True)
    add('domain_tail',[0.,0.,0.],reverse=True,domain=[-3.,17.])
    add('domain_inner',[5.,0.,0.],domain=[-3.,17.])
    add('tilted_inner',tilted=True,actions=[dict(grip=3)])
    add('tilted_tail',tilted=True,reverse=True,actions=[dict(grip=6)])
    add('reverse_post',[0.,0.,0.],reverse=True,preselect=False)
    add('domain_shared',[3.,0.,0.],reverse=True,domain=[-3.,17.])
    add('domain_negative_shared',[3.,0.,0.],reverse=True,domain=[-17.,3.])
    add('domain_small_shared',[3.,0.,0.],reverse=True,domain=[-5.,1.])
    add('near_arc_joint',[3.001,0.,0.],reverse=True)
    add('near_line_joint',[2.999,0.,0.],reverse=True)
    add('near_joint_above',[3.,.001,0.],reverse=True)
    add('near_joint_below',[3.,-.001,0.],reverse=True)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=rows)
    validate_operations(q)
    return q


def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations'
    (folder/'remove_control_point_composites.request.json').write_text(json.dumps(q,indent=2)+'\n')
    started=time.monotonic()
    r=OracleClient(settings_scheme='VibocerosOracleControlComposites20261019').run_rhino(q,timeout=300.)
    if [v['id']for v in r['results']] != [v['id']for v in q['operations']]:raise ValueError('incomplete composite capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_remove_control_point_composites.py','remove_control_point_probe.py','remove_knot_command_probe.py',
           'insert_kink_probe.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        seconds=time.monotonic()-started,sources={n:digest(Path(__file__).with_name(n))for n in names},
        fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (folder/'remove_control_point_composites.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'composite control removals',flush=True)


if __name__=='__main__':main()
