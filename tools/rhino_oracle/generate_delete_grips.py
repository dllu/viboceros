"""Capture selected-grip deletion, object precedence and history."""
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
    folder = ROOT / 'tools/rhino_oracle/fixtures/delete_grips'; folder.mkdir(exist_ok=True)
    operations = []
    def source(kind, selected='middle', mode='control', parent=False, hidden='empty'):
        return dict(kind=kind, selected=selected, mode=mode, parent=parent, hidden=hidden)
    def add(name, sources, peer=False, off=False):
        operations.append(dict(op='delete_grips', id=name, sources=sources, peer=peer, off=off, artifact_path=str(folder/(name+'.3dm'))))
    for kind in KINDS:
        if pilot and kind not in KINDS[:14]: continue
        seeds = ('middle', 'all') if pilot else ('first','middle','last','ends','adjacent','alternate','except_first','except_ends','all','empty')
        for seed in seeds: add(kind+'_'+seed, [source(kind, seed)])
        if not kind.startswith('surface') and not kind.startswith('mesh'):
            for seed in (('middle',) if pilot else ('first','middle','ends','all')):
                add(kind+'_edit_'+seed, [source(kind, seed, 'edit')])
        if kind.startswith('surface'):
            for seed in (('row_u','row_v')if pilot else('row_u','row_v','cross_rows')): add(kind+'_'+seed, [source(kind,seed)])
    for parent, peer in ((True,False),(False,True),(True,True)):
        add('mixed_'+str(int(parent))+str(int(peer)), [source(k,parent=parent) for k in ('curve','surface','mesh')], peer=peer)
    for mode in ('control','edit'):
        add('off_'+mode, [source('curve',mode=mode)], off=True)
        add('hidden_'+mode, [source('curve', selected='first', mode=mode, hidden='middle')])
        add('hidden_all_'+mode, [source('curve', selected='all', mode=mode, hidden='all')])
    add('logical_hidden', [source('curve', selected='neighbor_hidden', hidden='middle')])
    if not pilot:
        add('mixed_noop', [source('line',parent=True),source('mesh_fan','first'),source('weighted','first'),source('surface','row_u')],peer=True)
        add('mixed_objects', [source('curve','empty',parent=True),source('surface','row_v'),source('mesh_grid','first')],peer=True)
        add('surface_hidden_row', [source('surface','row_u',hidden='middle')])
        add('mesh_hidden', [source('mesh_fan','all',hidden='first')])
        add('only_hidden', [source('curve',selected='next_hidden',hidden='middle')])
        add('parent_only_hidden', [source('curve',selected='next_hidden',hidden='middle',parent=True)])
        add('parent_hidden_all', [source('curve',selected='empty',hidden='all',parent=True)])
        for kind in ('polycurve','polycurve_curved'):
            for seed in ('second','before_last'): add(kind+'_'+seed,[source(kind,seed)])
    q = dict(protocol_version=1, iterations=1, operations=operations); validate_operations(q); return q


def main():
    parser = argparse.ArgumentParser(__doc__); parser.add_argument('--pilot', action='store_true'); args = parser.parse_args()
    q = request(args.pilot); folder = ROOT/'tools/rhino_oracle'; stem = 'delete_grips_pilot' if args.pilot else 'delete_grips'
    path = folder/'fixtures'/(stem+'.json'); path.write_text(json.dumps(q,indent=2)+'\n')
    r = OracleClient(settings_scheme='VibocerosOracleDeleteGrips20261019').run_rhino(q,timeout=1200.)
    if [o['id'] for o in q['operations']] != [o['id'] for o in r['results']]: raise ValueError('incomplete Delete capture')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    names = ('generate_delete_grips.py','delete_grips_probe.py','grip_navigation_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance'] = dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(), request_sha256=digest(path),
        sources={n:digest(folder/n) for n in names}, fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path'])) for o in q['operations']})
    diagnostics = {o['id']:Path(o['artifact_path']+'.delete.log').read_text() for o in q['operations']}
    diagnostic_path = folder/'observations'/(stem+'_history.json'); diagnostic_path.write_text(json.dumps(diagnostics,indent=2)+'\n')
    r['provenance']['diagnostics_sha256'] = digest(diagnostic_path)
    (folder/'observations'/(stem+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for o in q['operations']:
        Path(o['artifact_path']+'.capture.json').unlink(); Path(o['artifact_path']+'.delete.log').unlink()
    print('Recorded',len(r['results']),'Delete workflows',flush=True)


if __name__ == '__main__': main()
