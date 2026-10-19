"""Capture every ModifyRadius size mode against native source 3dm files."""
from pathlib import Path
import hashlib
import json
import subprocess
import time
from .client import OracleClient
from .modify_radius_probe import FAMILIES, MODES
from .structural_sdk import validate_operations

ROOT = Path(__file__).resolve().parents[2]


def request():
    folder = ROOT/'tools/rhino_oracle/fixtures/modify_radius'
    folder.mkdir(exist_ok=True)
    def op(name, family='arc', mode='Radius', value=4., **kw):
        return dict(op='modify_radius_command', id=name, family=family, mode=mode, value=value,
                    artifact_path=str(folder/(name+'.3dm')), **kw)
    ops = [op(f, family=f) for f in FAMILIES if f != 'near_circle']
    for mode, value in [('Radius', 4.), ('Diameter', 8.), ('Area', 16.*3.141592653589793), ('Circumference', 8.*3.141592653589793)]:
        ops.extend([op(mode.lower()+'_copy', mode=mode, value=value, copy=True),
                    op(mode.lower()+'_post', mode=mode, value=value, preselect=False),
                    op(mode.lower()+'_post_copy', mode=mode, value=value, preselect=False, copy=True)])
    for family in ('arc', 'circle', 'nurbs_arc', 'nurbs_circle'):
        ops.extend([op(family+'_domain', family=family, domain=[-3., 7.]),
                    op(family+'_same', family=family, value=2.),
                    op(family+'_same_copy', family=family, value=2., copy=True)])
    ops.extend([op('batch', family='nurbs_circle', copies=3), op('batch_copy', family='nurbs_arc', copies=3, copy=True),
                op('mixed', mixed=True), op('tiny', value=1e-8), op('large', family='circle', value=1e6)])
    # Probe recognition independently of the document modelling tolerance.
    for delta, label in [(1e-12, 'tiny'), (1e-8, 'small'), (1e-6, 'model'), (1e-3, 'outside')]:
        ops.append(op('near_'+label, family='near_circle', delta=delta))
    q = dict(protocol_version=1, iterations=1, tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6), operations=ops)
    validate_operations(q)
    return q


def main():
    q = request(); folder = ROOT/'tools/rhino_oracle/observations'
    (folder/'modify_radius.request.json').write_text(json.dumps(q, indent=2)+'\n')
    start = time.monotonic()
    r = OracleClient(settings_scheme='VibocerosOracleModifyRadius20261018V3').run_rhino(q, timeout=300.)
    if len(r['results']) != len(q['operations']) or not all(v['value']['command_result'] in ('Success', 'Nothing') for v in r['results']):
        raise ValueError('incomplete ModifyRadius capture')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    files = ['modify_radius_probe.py', 'generate_modify_radius.py', 'rhino_worker.py', 'structural_sdk.py', 'join_probe.py', 'merge_edges_probe.py']
    r['provenance'] = dict(base_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        seconds=time.monotonic()-start, sources={f: digest(Path(__file__).with_name(f)) for f in files},
        fixtures={Path(o['artifact_path']).name: digest(Path(o['artifact_path'])) for o in q['operations']})
    (folder/'modify_radius.json').write_text(json.dumps(r, indent=2, allow_nan=False)+'\n')
    print('Recorded', len(r['results']), 'actual ModifyRadius commands', flush=True)


if __name__ == '__main__':
    main()
