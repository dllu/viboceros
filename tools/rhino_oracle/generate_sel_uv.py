"""Bounded public SelUV input and display witnesses."""
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations

ROOT = Path(__file__).resolve().parents[2]


def request():
    folder = ROOT / 'tools/rhino_oracle/fixtures/select_uv_rows'
    folder.mkdir(exist_ok=True)
    operations = []
    def add(name, family, actions, **kw):
        defaults = [] if kw.get('cancel_surface') else ['Direction=U', 'HideOthers=No']
        operations.append(dict(op='select_uv_rows', id=name, family=family,
            actions=defaults + actions, artifact_path=str(folder / (name + '.3dm')), **kw))
    for family in ('surface','surface_closed','surface_periodic_u','surface_periodic_v'):
        for direction in ('U','V','Both'):
            add(family+'_'+direction, family, ['Direction='+direction,dict(grip=4)])
            add(family+'_hide_'+direction, family, ['Direction='+direction,'HideOthers=Yes',dict(grip=4)])
        add(family+'_repeat', family, [dict(grip=4),dict(grip=0),dict(grip=4)])
        add(family+'_switch', family, [dict(grip=0),'Toggle',dict(grip=4)])
    for direction in ('U','V','Both'):
        add('toggle_'+direction,'surface',['Direction='+direction,'Toggle',dict(grip=4)])
    for on in (False,True):
        add('empty_'+str(int(on)),'surface',[],grips_on=on,selected=[0] if on else [])
        add('cancel_'+str(int(on)),'surface',[dict(grip=4)],grips_on=on,selected=[0] if on else [],cancel=True)
        add('cancel_surface_'+str(int(on)),'surface',[],grips_on=on,selected=[0] if on else [],cancel_surface=True)
        add('preselected_'+str(int(on)),'surface',[dict(grip=4)],grips_on=on,preselected=True)
    add('hide_empty','surface',['HideOthers=Yes'])
    add('hide_cancel','surface',['HideOthers=Yes',dict(grip=4)],cancel=True)
    add('hide_restore','surface',['HideOthers=Yes',dict(grip=4),'HideOthers=No'])
    add('direction_memory','surface',['Direction=V',dict(grip=4)],next_actions=[dict(grip=4)])
    add('cancel_memory','surface',['Direction=Both','HideOthers=Yes',dict(grip=4)],cancel=True,next_actions=[dict(grip=0)])
    for i,p in enumerate(([.4,.49,.1],[.5,1.49,.2],[.5,1.51,.2],[1.49,1.51,5.],[.1,.6,0.])):
        add('world_'+str(i),'surface',[dict(world=p)])
    for x in [.1,.4,.8,1.2]:
        for y in [.35,.45,.55,.65,.75,1.2,1.4,1.6]:
            add('grid_'+str(sum(o['id'].startswith('grid_') for o in operations)), 'surface', [dict(world=[x,y,.1])])
    for family in ('surface_nonuniform','surface_cubic'):
        for i,uv in enumerate(([.05,.1],[.2,.3],[.35,.45],[.55,.65],[.8,.9],[1.,1.])):
            for direction in ('U','V','Both'):
                add(family+'_'+str(i)+'_'+direction, family, ['Direction='+direction,dict(parameter=uv)])
    add('preselected_seed','surface',[dict(grip=4)],grips_on=True,selected=[0],preselected=True)
    add('managed_hide','surface',['HideOthers=Yes',dict(grip=4),'Direction=V',dict(grip=0),'HideOthers=No'],managed=True)
    add('managed_cancel','surface',['HideOthers=Yes',dict(grip=4)],managed=True,cancel=True)
    add('managed_both','surface',['Direction=Both','HideOthers=Yes',dict(grip=4)],managed=True)
    q = dict(protocol_version=1, iterations=1, operations=operations)
    validate_operations(q)
    return q


def main():
    q = request(); folder = ROOT / 'tools/rhino_oracle'
    path = folder / 'fixtures/select_uv_rows.json'; path.write_text(json.dumps(q, indent=2) + '\n')
    r = OracleClient(settings_scheme='VibocerosOracleSelUV20261019').run_rhino(q, timeout=600.)
    if [x['id'] for x in r['results']] != [x['id'] for x in q['operations']]: raise ValueError('incomplete SelUV capture')
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    names = ('generate_sel_uv.py','sel_uv_probe.py','sel_uv_input.py','grip_navigation_probe.py','grip_transform_probe.py',
             'join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance'] = dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
        request_sha256=digest(path), sources={n:digest(Path(__file__).with_name(n)) for n in names},
        fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path'])) for o in q['operations']})
    (folder / 'observations/select_uv_rows.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for op in q['operations']: Path(op['artifact_path']+'.capture.json').unlink()
    print('Recorded',len(r['results']),'SelUV workflows',flush=True)


if __name__ == '__main__': main()
