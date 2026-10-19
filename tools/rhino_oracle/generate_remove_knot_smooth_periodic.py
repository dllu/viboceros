"""Repeated merged polycurve charts and genuinely deforming periodic tensors."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
from .client import OracleClient
from .generate_remove_knot_command import request as base_request, ROOT
from .structural_sdk import validate_operations

def request():
    from .generate_remove_knot_followup import request as followup
    q=followup()
    folder=ROOT/'tools/rhino_oracle/fixtures/remove_knot_smooth_periodic'
    folder.mkdir(exist_ok=True)
    q['operations']=[dict(op,op='remove_knot_smooth_periodic',artifact_path=str(folder/(op['id']+'.3dm')))for op in q['operations']if 'tensor'in op['id']]
    validate_operations(q)
    return q

def main():
    q = request()
    folder = ROOT / 'tools/rhino_oracle/observations'
    (folder/'remove_knot_smooth_periodic.request.json').write_text(json.dumps(q,indent=2)+'\n')
    started = time.monotonic()
    r = OracleClient(settings_scheme='VibocerosOracleRemoveKnotSmooth20261019').run_rhino(q,timeout=600.)
    if [x['id'] for x in r['results']] != [x['id'] for x in q['operations']]:
        raise ValueError('incomplete followup capture')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    names = ('remove_knot_command_probe.py','generate_remove_knot_command.py',
             'generate_remove_knot_followup.py','generate_remove_knot_smooth_periodic.py','remove_knot_smooth_periodic_probe.py','insert_kink_probe.py','rhino_worker.py',
             'structural_sdk.py','join_probe.py','merge_edges_probe.py')
    r['provenance'] = dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        seconds=time.monotonic()-started, sources={n:digest(Path(__file__).with_name(n)) for n in names},
        fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path'])) for o in q['operations']})
    (folder/'remove_knot_smooth_periodic.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'RemoveKnot followup commands',flush=True)
if __name__ == '__main__':
    main()
