"""Full public removal getters, including deformation, trims and periodic sources."""
import hashlib,json,subprocess,time
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
ROOT=Path(__file__).resolve().parents[2]
def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/remove_knot_command';folder.mkdir(exist_ok=True)
    def point(f):return dict(point=f)
    def op(name,family,actions=None,**kw):return dict(op='remove_knot_command',id=name,family=family,actions=[point(.5)]if actions is None else actions,seed=True,artifact_path=str(folder/(name+'.3dm')),**kw)
    rows=[]
    for family in('cubic','rational','line','polyline','circle','ellipse','polycurve','periodic'):
        rows.extend([op(family,family),op(family+'_deformed',family,deform=True),op(family+'_repeat',family,[point(.5)]*3),op(family+'_ends',family,[point(0.),point(1.)]),op(family+'_cancel',family,cancel=True),op(family+'_empty',family,[]),op(family+'_post',family,preselect=False),op(family+'_undo',family,[point(.2),point(.8),'Undo']),op(family+'_undo_all',family,[point(.5),'Undo']),op(family+'_numeric',family,[dict(parameter=.5)])])
        raw=op(family+'_unseeded',family);raw['seed']=False;rows.append(raw)
    for family in('surface','surface_rational'):
        for direction in('U','V','U_to_V'):
            actual='U'if direction=='U_to_V'else direction
            prefix=['Toggle']if direction=='U_to_V'else[]
            rows.extend([op(family+'_'+direction,family,prefix+[point(.5)],direction=actual),op(family+'_'+direction+'_deformed',family,prefix+[point(.5)],direction=actual,deform=True),op(family+'_'+direction+'_repeat',family,prefix+[point(.5)]*3,direction=actual),op(family+'_'+direction+'_undo',family,prefix+[point(.2),point(.8),'Undo'],direction=actual)])
        rows.extend([op(family+'_ends',family,[point(0.),point(1.)]),op(family+'_cancel',family,cancel=True),op(family+'_empty',family,[]),op(family+'_post',family,preselect=False),op(family+'_toggle',family,['Toggle',point(.5)]),op(family+'_numeric',family,[dict(parameter=.5)])])
    for family in('trimmed_rectangle','trimmed_hole'):
        rows.extend([op(family,family,direction='U'),op(family+'_deformed',family,direction='U',deform=True),op(family+'_cancel',family,direction='U',cancel=True),op(family+'_center',family,[point([.5,.5])],direction='U'),op(family+'_deformed_center',family,[point([.5,.5])],direction='U',deform=True)])
    for family in('cubic','rational','circle'):
        rows.append(op(family+'_subcurve',family,subcurve=[.1,.9],preselect=False))
    for axis in('U','V'):
        for direction in('U','V','U_to_V'):rows.append(op('periodic_surface_'+axis+'_'+direction,'surface',(['Toggle']if direction=='U_to_V'else[])+[point(.5)],periodic_surface_axis=axis,direction='U'if direction=='U_to_V'else direction))
    rows.extend([op('elevated','cubic',elevate=5,deform=True),op('negative_gauge','rational',gauge=-8.,deform=True),op('domain','rational',[dict(parameter=7.)],domain=[-3.,17.],deform=True),op('tilted','cubic',tilted=True,deform=True)])
    joint=op('polycurve_joint','polycurve',[point(3./(3.+2.*3.141592653589793))]);joint['seed']=False;rows.append(joint)
    box=op('polysurface','box',face=2);box['seed']=False;rows.append(box)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=rows);validate_operations(q);return q
def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations';(folder/'remove_knot_command.request.json').write_text(json.dumps(q,indent=2)+'\n');started=time.monotonic()
    r=OracleClient(settings_scheme='VibocerosOracleRemoveKnot20261019').run_rhino(q,timeout=600.)
    if [x['id']for x in r['results']]!=[x['id']for x in q['operations']]:raise ValueError('incomplete removal capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();names=('remove_knot_command_probe.py','generate_remove_knot_command.py','insert_kink_probe.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-started,sources={n:digest(Path(__file__).with_name(n))for n in names},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (folder/'remove_knot_command.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded',len(r['results']),'actual RemoveKnot commands',flush=True)
if __name__=='__main__':main()
