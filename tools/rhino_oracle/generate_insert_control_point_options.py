"""Retain native rejection of the help page's unsupported Both option."""
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .generate_remove_control_point import ROOT
from .structural_sdk import validate_operations


def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/insert_control_point_options';folder.mkdir(exist_ok=True)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
        operations=[dict(op='insert_control_point_command',id='reject_both',family='surface',direction='Both',
            actions=['Direction=U',dict(point=[.4,.6])],artifact_path=str(folder/'reject_both.3dm'))])
    validate_operations(q)
    return q


def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations'
    (folder/'insert_control_point_options.request.json').write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleInsertControlOptions20261019').run_rhino(q,timeout=300.)
    if [x['id']for x in r['results']]!=['reject_both']:raise ValueError('incomplete option capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_insert_control_point_options.py','insert_control_point_probe.py',
        'remove_knot_command_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        sources={n:digest(Path(__file__).with_name(n))for n in names},fixtures={'reject_both.3dm':digest(Path(q['operations'][0]['artifact_path']))})
    (folder/'insert_control_point_options.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded native Both rejection',flush=True)


if __name__=='__main__':main()
