"""Analytic source charts must survive conversion during insertion."""
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .generate_remove_control_point import ROOT
from .structural_sdk import validate_operations


def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/insert_control_point_charts';folder.mkdir(exist_ok=True)
    rows=[]
    for name,reverse,midpoint in [('circle_domain',False,False),('circle_reverse_domain',True,False),('circle_midpoint_domain',False,True)]:
        actions=(['Midpoint=Yes']if midpoint else[])+[dict(point=.4)]
        rows.append(dict(op='insert_control_point_command',id=name,family='circle',actions=actions,
            reverse=reverse,domain=[-3.,17.],artifact_path=str(folder/(name+'.3dm'))))
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=rows)
    validate_operations(q)
    return q


def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations'
    (folder/'insert_control_point_charts.request.json').write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleInsertControlCharts20261019').run_rhino(q,timeout=300.)
    if [x['id']for x in r['results']]!=[o['id']for o in q['operations']]:raise ValueError('incomplete chart capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_insert_control_point_charts.py','insert_control_point_probe.py',
        'remove_knot_command_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        sources={n:digest(Path(__file__).with_name(n))for n in names},
        fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (folder/'insert_control_point_charts.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'InsertControlPoint analytic charts',flush=True)


if __name__=='__main__':main()
