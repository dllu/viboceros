"""Bounded HidePt/ShowPt, selection and grip-mode lifecycle witnesses."""
import hashlib
import json
import subprocess
from pathlib import Path
from .client import OracleClient
from .grip_visibility_probe import KINDS
from .structural_sdk import validate_operations

ROOT=Path(__file__).resolve().parents[2]


def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/grip_visibility';folder.mkdir(exist_ok=True)
    operations=[]
    def add(name,sources):
        actions=[dict(command='HidePt',seed='first'),dict(command='ShowPt'),dict(command='HidePt',seed='empty'),dict(command='ShowPt'),dict(command='HidePt',seed='empty',pick='all',cancel=True),dict(command='HidePt',seed='empty',pick='all'),dict(command='ShowPt'),dict(command='HidePt',seed='first'),
            dict(command='SelPt'),dict(command='InvertPt'),dict(command='ShowPt'),dict(command='HidePt',seed='middle'),dict(command='HidePt',seed='middle')]
        actions += [dict(command=c,seed=s)for c,s in [('NextU','first'),('NextU','before_middle'),('PrevU','last'),('AddNextU','before_middle'),('AddPrevU','last'),('NextV','before_middle'),('PrevV','last'),('AddNextV','before_middle'),('AddPrevV','last'),('SelU','first'),('SelV','last')]]
        actions += [dict(command='ShowPt'),dict(command='HidePt',seed='all'),dict(command='HidePt',seed='middle'),dict(command='SelPt'),dict(command='InvertPt'),dict(command='NextU'),
            dict(command='ShowPt'),dict(command='ShowPt'),dict(command='HidePt',seed='ends'),dict(command='HidePt',seed='empty'),dict(command='ShowPt'),
            dict(command='HidePt',seed='middle'),dict(command='PointsOn',parents=True),dict(command='ShowPt')]
        if all(s['mode']=='edit'or not s['kind'].startswith('surface')and s['kind']!='mesh'for s in sources):
            actions += [dict(command='HidePt',seed='middle'),dict(command='EditPtOn',parents=True),dict(command='ShowPt')]
        if len(sources)==1 and sources[0]['kind'].startswith('surface'):
            actions += [dict(command='HidePt',seed='middle'),dict(command='SelUV'),dict(command='ShowPt')]
        actions += [dict(command='HidePt',seed='middle'),dict(command='PointsOff'),dict(command='ShowPt'),dict(command='PointsOn',parents=True),dict(command='ShowPt')]
        operations.append(dict(op='grip_visibility',id=name,sources=sources,actions=actions,artifact_path=str(folder/(name+'.3dm'))))
    for kind in KINDS:
        for mode in ('control','edit'):
            if mode=='edit'and(kind.startswith('surface')or kind=='mesh'):continue
            add(kind+'_'+mode,[dict(kind=kind,mode=mode,parent=False)])
    for parent in (False,True):
        add('mixed_'+str(int(parent)),[dict(kind=k,mode=m,parent=parent)for k,m in [('periodic','control'),('surface_closed','control'),('curve','edit'),('mesh','control')]])
    for mode in ('control','edit'):
        for mixed in (False,True):
            name=('mixed_hidden_hide_' if mixed else 'hidden_selected_hide_')+mode
            actions=[dict(command='SelNone'),dict(command='HidePt',seed='middle'),dict(command='AddNextU'if mixed else 'NextU',seed='before_middle'),dict(command='HidePt'),dict(command='ShowPt')]
            operations.append(dict(op='grip_visibility',id=name,sources=[dict(kind='curve',mode=mode,parent=True)],actions=actions,artifact_path=str(folder/(name+'.3dm'))))
    for mode in ('control','edit'):
        for case in ('mixed_move','hidden_move','visible_move_history','mixed_move_history','visible_move_hide_history'):
            actions=[dict(command='SelNone'),dict(command='HidePt',seed='middle')]
            if case.startswith('mixed'):actions.append(dict(command='AddNextU',seed='before_middle'))
            elif case=='hidden_move':actions.append(dict(command='NextU',seed='before_middle'))
            else:actions.append(dict(command='SelNone'))
            actions.append(dict(command='Move',**({'seed':'first'}if case.startswith('visible')else{})))
            if 'history'in case:actions += [dict(command='Undo'),dict(command='HidePt'if 'hide_history'in case else'ShowPt'),dict(command='Redo')]
            name='edited_'+case+'_'+mode
            operations.append(dict(op='grip_visibility',id=name,sources=[dict(kind='curve',mode=mode,parent=False)],actions=actions,artifact_path=str(folder/(name+'.3dm'))))
    q=dict(protocol_version=1,iterations=1,operations=operations);validate_operations(q);return q


def main():
    q=request();folder=ROOT/'tools/rhino_oracle';path=folder/'fixtures/grip_visibility.json';path.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleGripVisibility20261019').run_rhino(q,timeout=1200.)
    if [o['id']for o in q['operations']]!=[o['id']for o in r['results']]:raise ValueError('incomplete visibility capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('generate_grip_visibility.py','grip_visibility_probe.py','hide_pt_input.py','grip_navigation_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),request_sha256=digest(path),
        sources={n:digest(Path(__file__).with_name(n))for n in names},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (folder/'observations/grip_visibility.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for op in q['operations']:Path(op['artifact_path']+'.capture.json').unlink()
    print('Recorded',len(r['results']),'grip visibility workflows',flush=True)


if __name__=='__main__':main()
