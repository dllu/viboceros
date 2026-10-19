"""Endpoint, closed-seam and composite-junction insertion decisions."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from .client import OracleClient
from .generate_remove_control_point import ROOT
from .structural_sdk import validate_operations


def request():
    folder = ROOT / 'tools/rhino_oracle/fixtures/insert_control_point_endpoints'
    folder.mkdir(exist_ok=True)
    rows = []
    def add(name, family, actions, **kw):
        rows.append(dict(op='insert_control_point_command', id=name, family=family,
                         actions=actions, artifact_path=str(folder/(name+'.3dm')), **kw))
    for family in ('cubic', 'rational', 'circle', 'ellipse', 'periodic'):
        for side in (0., 1.):
            add(family+'_'+str(int(side)), family, [dict(point=side)])
    for side in (0., 1.):
        add('seeded_'+str(int(side)), 'cubic', [dict(point=side)], seed=True)
    add('seeded_midpoint', 'cubic', ['Midpoint=Yes', dict(point=0.)], seed=True)
    for reverse in (False, True):
        add('joint_'+str(int(reverse)), 'polycurve', [dict(world=[3.,0.,0.])], reverse=reverse)
    q=dict(protocol_version=1, iterations=1,
           tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=rows)
    validate_operations(q)
    return q


def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations'
    (folder/'insert_control_point_endpoints.request.json').write_text(json.dumps(q,indent=2)+'\n')
    started=time.monotonic()
    r=OracleClient(settings_scheme='VibocerosOracleInsertControlEndpoints20261019').run_rhino(q,timeout=600.)
    if [x['id']for x in r['results']]!=[o['id']for o in q['operations']]:raise ValueError('incomplete endpoint capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_insert_control_point_endpoints.py','insert_control_point_probe.py',
        'remove_knot_command_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        seconds=time.monotonic()-started,sources={n:digest(Path(__file__).with_name(n))for n in names},
        fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (folder/'insert_control_point_endpoints.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'InsertControlPoint endpoint decisions',flush=True)


if __name__=='__main__':main()
