"""Capture Weight on shared curve/surface controls, hidden picks and history."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .delete_grips_probe import KINDS
from .structural_sdk import validate_operations

ROOT = Path(__file__).resolve().parents[2]


def request(pilot=False):
    stem = 'grip_weight_pilot3' if pilot else 'grip_weight'
    folder = ROOT / 'tools/rhino_oracle/fixtures' / stem
    folder.mkdir(exist_ok=True)
    operations = []
    def source(kind, selected='middle', mode='control', parent=False, hidden='empty'):
        return dict(kind=kind, selected=selected, mode=mode, parent=parent, hidden=hidden)
    def add(name, sources, inputs=None, dash=True, peer=False, off=False, pick=False, repeat=False):
        operations.append(dict(op='grip_weight', id=name, artifact_path=str(folder/(name+'.3dm')),
            sources=sources, inputs=[2.] if inputs is None else inputs, dash=dash, peer=peer, off=off,pick=pick,repeat=repeat))
    kinds = ('weighted','periodic','closed_weighted','surface_closed','mesh','polycurve','polycurve_curved') if pilot else KINDS
    for kind in kinds:
        for seed in (('middle','ends') if pilot else ('first','middle','ends','all','empty')):
            add(kind+'_'+seed, [source(kind, seed)])
        if not kind.startswith(('surface','mesh')):
            add(kind+'_edit_middle', [source(kind, mode='edit')])
        if kind.startswith('surface'):
            add(kind+'_row_u', [source(kind, 'row_u')])
    if not pilot:
        for kind in kinds:
            add(kind+'_default', [source(kind,'first')], inputs=['Enter'])
    for kind in ('curve','weighted','mesh'):
        for hidden, selected in [('middle','all'),('middle','next_hidden'),('all','empty')]:
            if selected=='next_hidden' and kind!='curve': continue
            add(kind+'_'+hidden+'_'+selected, [source(kind, selected, hidden=hidden)], peer=True)
    for kind in ('curve','weighted','surface','mesh'):
        add('pick_'+kind, [source(kind,'empty')], pick=True)
    for mode in ('control','edit'):
        add('off_'+mode, [source('weighted', mode=mode)], off=True)
    for mode in ('control','edit'):
        add('cancel_'+mode, [source('weighted',mode=mode)], inputs=['Cancel'], peer=True)
    for kind in ('curve','weighted','polycurve_curved'):
        add('default_'+kind, [source(kind,'all')], inputs=['Enter'])
        add('same_'+kind, [source(kind,'first')], inputs=[.7 if kind=='weighted' else 1.])
    for inputs in ([.001],[100.],[-1.,2.],[0.,2.],[-1.,'Cancel']):
        add('number_'+str(len(operations)), [source('weighted')], inputs=inputs)
    for kind in ('weighted','polycurve_curved'):
        add('dialog_'+kind, [source(kind,'middle')], dash=False)
        add('dialog_cancel_'+kind, [source(kind,'middle')], inputs=[2.,'Cancel'], dash=False)
    if not pilot:
        for kind in ('weighted','curve','line','polyline','polycurve','polycurve_curved','surface','closed_weighted'):
            add('repeat_'+kind, [source(kind,'ends')], repeat=True)
        add('mixed_equal', [source(k,parent=True)for k in ('weighted','curve','surface','polycurve_curved')], inputs=[1.],peer=True)
    add('mixed', [source(k,parent=True)for k in ('weighted','surface','mesh','polycurve_curved')], peer=True)
    q = dict(protocol_version=1, iterations=1, operations=operations)
    validate_operations(q)
    return q


def main():
    parser=argparse.ArgumentParser(__doc__);parser.add_argument('--pilot',action='store_true');args=parser.parse_args()
    stem='grip_weight_pilot3' if args.pilot else 'grip_weight';h=ROOT/'tools/rhino_oracle'
    q=request(args.pilot);path=h/'fixtures'/(stem+'.json');path.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleWeight20261019').run_rhino(q,timeout=900.)
    if [o['id']for o in q['operations']] != [o['id']for o in r['results']]:raise ValueError('incomplete Weight capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_grip_weight.py','grip_weight_probe.py','weight_dialog.py','delete_grips_probe.py','grip_navigation_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(path),sources={n:digest(h/n)for n in names},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    diagnostics={o['id']:json.loads(Path(o['artifact_path']+'.weight.log').read_text())for o in q['operations']}
    diagnostic_path=h/'observations'/(stem+'_input.json');diagnostic_path.write_text(json.dumps(diagnostics,indent=2)+'\n');r['provenance']['diagnostics_sha256']=digest(diagnostic_path)
    (h/'observations'/(stem+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for o in q['operations']:
        Path(o['artifact_path']+'.capture.json').unlink();Path(o['artifact_path']+'.weight.log').unlink()
    print('Recorded',len(r['results']),'Weight workflows',flush=True)


if __name__=='__main__':main()
