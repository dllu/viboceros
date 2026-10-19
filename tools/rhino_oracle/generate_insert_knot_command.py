"""Capture real InsertKnot point, numeric, Automatic and local-Undo commands."""
import hashlib,json,subprocess,time
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
ROOT=Path(__file__).resolve().parents[2]
def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/insert_knot_command';folder.mkdir(exist_ok=True)
    def point(f):return dict(point=f)
    def op(name,family,actions=None,**kw):return dict(op='insert_knot_command',id=name,family=family,actions=[point(.35)]if actions is None else actions,artifact_path=str(folder/(name+'.3dm')),**kw)
    out=[]
    for family in('cubic','rational','circle','ellipse','line','polyline','polycurve','periodic'):
        for name,actions,kw in [('',[point(.35)],{}),('_two',[point(.35),point(.7)],{}),('_repeat',[point(.35)]*4,{}),('_ends',[point(0.),point(1.)],{}),('_empty',[],{}),('_cancel',[point(.35)],dict(cancel=True)),('_post',[point(.35)],dict(preselect=False)),('_auto',['Automatic'],{}),('_auto_twice',['Automatic','Automatic'],{}),('_undo',[point(.35),point(.7),'Undo'],{}),('_undo_all',[point(.35),'Undo'],{}),('_numeric',[dict(parameter=.35)],{}),('_sym',[point(.35)],dict(symmetrical=True))]:
            out.append(op(family+name,family,actions,**kw))
    for family in('surface','surface_rational'):
        for direction in('U','V','Both'):
            for symmetry in(False,True):out.append(op(family+'_'+direction+('_sym'if symmetry else''),family,direction=direction,symmetrical=symmetry))
            out.append(op(family+'_auto_'+direction,family,['Automatic'],direction=direction))
        out.extend([op(family+'_auto_twice',family,['Automatic','Automatic'],direction='Both'),op(family+'_undo',family,[point(.35),point(.7),'Undo'],direction='Both'),op(family+'_toggle',family,['Toggle',point(.35)]),op(family+'_cancel',family,cancel=True),op(family+'_post',family,preselect=False),op(family+'_ends',family,[point(0.),point(1.)]),op(family+'_midpoints',family,midpoints=True)])
    for family in('trimmed_rectangle','trimmed_hole'):
        out.extend([op(family,family,direction='Both'),op(family+'_auto',family,['Automatic'],direction='Both'),op(family+'_cancel',family,cancel=True)])
    for family in('cubic','circle','rational'):
        out.extend([op(family+'_subcurve',family,subcurve=[.2,.8],preselect=False),op(family+'_subcurve_auto',family,['Automatic'],subcurve=[.2,.8],preselect=False)])
    out.extend([op('elevated','cubic',elevate=5),op('elevated_rational','rational',elevate=7),op('domain','rational',[dict(parameter=7.)],domain=[-3.,17.]),op('reversed','rational',reverse=True),op('negative_gauge','rational',gauge=-8.),op('tilted','cubic',tilted=True),op('refined','cubic',refine=.5),op('polysurface','box',face=2)])
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=out);validate_operations(q);return q
def capture(q,destination):
    started=time.monotonic();r=OracleClient(settings_scheme='VibocerosOracleInsertKnot20261019').run_rhino(q,timeout=600.)
    if [v['id']for v in r['results']]!=[v['id']for v in q['operations']]:raise ValueError('incomplete knot capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('insert_knot_command_probe.py','generate_insert_knot_command.py','insert_kink_probe.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-started,sources={n:digest(Path(__file__).with_name(n))for n in names},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    Path(destination).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded',len(r['results']),'actual InsertKnot commands',flush=True)
def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations';(folder/'insert_knot_command.request.json').write_text(json.dumps(q,indent=2)+'\n');capture(q,folder/'insert_knot_command.json')
if __name__=='__main__':main()
