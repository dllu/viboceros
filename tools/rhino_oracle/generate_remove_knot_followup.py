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
    q = base_request()
    sources = {o['id']: o for o in q['operations']}
    folder = ROOT / 'tools/rhino_oracle/fixtures/remove_knot_followup'
    folder.mkdir(exist_ok=True)
    rows = []
    def add(name, source, actions, **changes):
        op = dict(sources[source], id=name, actions=actions, artifact_path=str(folder / (name+'.3dm')), **changes)
        rows.append(op)
    point = sources['polycurve_joint']['actions'][0]
    add('joint_twice', 'polycurve_joint', [point]*2)
    add('joint_thrice', 'polycurve_joint', [point]*3)
    add('joint_undo', 'polycurve_joint', [point]*2+['Undo'])
    add('joint_undo_all', 'polycurve_joint', [point]*2+['Undo','Undo'])
    add('cyclic_tensor_repeat', 'periodic_surface_U_V', [dict(point=.5)]*2, deform=True)
    add('cyclic_tensor_undo', 'periodic_surface_V_U', [dict(point=.5)]*2+['Undo'], deform=True)
    q['operations'] = rows
    validate_operations(q)
    return q

def main():
    q = request()
    folder = ROOT / 'tools/rhino_oracle/observations'
    (folder/'remove_knot_followup.request.json').write_text(json.dumps(q,indent=2)+'\n')
    started = time.monotonic()
    r = OracleClient(settings_scheme='VibocerosOracleRemoveKnotFollowup20261019').run_rhino(q,timeout=600.)
    if [x['id'] for x in r['results']] != [x['id'] for x in q['operations']]:
        raise ValueError('incomplete followup capture')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    names = ('remove_knot_command_probe.py','generate_remove_knot_command.py',
             'generate_remove_knot_followup.py','insert_kink_probe.py','rhino_worker.py',
             'structural_sdk.py','join_probe.py','merge_edges_probe.py')
    r['provenance'] = dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        seconds=time.monotonic()-started, sources={n:digest(Path(__file__).with_name(n)) for n in names},
        fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path'])) for o in q['operations']})
    (folder/'remove_knot_followup.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'RemoveKnot followup commands',flush=True)
if __name__ == '__main__':
    main()
