"""Native parameter diagnostics, with all source definitions retained."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from .client import OracleClient
from .generate_remove_control_point import ROOT
from .structural_sdk import validate_operations


def request():
    q = json.loads((ROOT/'tools/rhino_oracle/observations/remove_control_point_neighbors.request.json').read_text())
    names = {'center_%d_exact_a' % i for i in range(6)} | {'center_%d_s0_a0_plus' % i for i in range(4)}
    folder = ROOT/'tools/rhino_oracle/fixtures/remove_control_point_parameters';folder.mkdir(exist_ok=True)
    q['operations'] = [dict(o, op='remove_control_point_parameters', artifact_path=str(folder/(o['id']+'.3dm'))) for o in q['operations'] if o['id'] in names]
    validate_operations(q)
    return q


def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations'
    (folder/'remove_control_point_parameters.request.json').write_text(json.dumps(q,indent=2)+'\n')
    started=time.monotonic()
    r=OracleClient(settings_scheme='VibocerosOracleControlParameters20261019').run_rhino(q,timeout=300.)
    if [x['id']for x in r['results']] != [x['id']for x in q['operations']]:raise ValueError('incomplete parameter capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('remove_control_point_parameters_probe.py','generate_remove_control_point_parameters.py',
           'remove_control_point_probe.py','remove_knot_command_probe.py','insert_kink_probe.py',
           'rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        seconds=time.monotonic()-started,sources={n:digest(Path(__file__).with_name(n))for n in names},
        fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (folder/'remove_control_point_parameters.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'control parameter queries',flush=True)


if __name__=='__main__':main()
