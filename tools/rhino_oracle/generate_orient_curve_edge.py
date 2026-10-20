"""Public curve-to-edge workflows with fixed scenes and prompt-gated input."""
import gzip,hashlib,json
from pathlib import Path
HERE=Path(__file__).resolve().parent

def request():
    xy=[[0,-6,0],[2,-6,0],[2,-5,0],[3,-5,0]]
    corners=[[0,-2,0],[12,-2,0],[12,6,0],[0,6,0]]
    cp=[[0,0,0],[1,0,0],[0,1,0]]
    ops=[]
    def add(name,points=xy,c=corners,plane=cp,end=False,flip=False,reverse=False,reversed_face=False,copy=False,grouped=False,kind='Nurbs',edge=0,finish=None):
        pick=points[-1]if end else points[0]
        aim=[[8,-2,0],[12,2,0],[8,6,0],[0,2,0]][edge]
        target=[[6,-2,0],[12,1,0],[6,6,0],[0,1,0]][edge]
        if c!=corners:aim=[8,-2,2];target=[6,-2,1.5]
        actions=[dict(curve=0,pick=pick),dict(curve=1,edge=edge,pick=aim),'Copy='+('Yes'if copy else'No'),'FlipSurface='+('Yes'if flip else'No'),'ReverseCurve='+('Yes'if reverse else'No'),target]
        prompts=[['Select curve to orient'],['Select target surface edge']]+[['Pick target edge point']]*4
        if copy:actions += [[9,-2,target[2]*1.5],'Cancel'];prompts += [['Pick target edge point']]*2
        if finish is not None:actions=actions[:finish]+['Cancel'];prompts=prompts[:finish]+[prompts[finish]]
        ops.append(dict(op='orient_curve_edge',id=name,kind=kind,grouped=grouped,points=points,corners=c,reversed=reversed_face,cplane=plane,actions=actions,action_prompts=prompts))
    for name,points in [('xy',xy),('xz',[[0,-6,0],[2,-6,0],[2,-6,1],[3,-6,1]]),('yz',[[0,-6,0],[0,-4,0],[0,-4,1],[0,-3,1]])]:
        for end in [False,True]:
            for flip,reverse in [(False,False),(True,False),(False,True),(True,True)]:add(name+'_'+str(int(end))+str(int(flip))+str(int(reverse)),points=points,end=end,flip=flip,reverse=reverse)
    for edge in [1,2,3]:add('boundary_'+str(edge),edge=edge)
    for name,c in [('tilted',[[0,-2,0],[12,-2,3],[12,6,3],[0,6,0]]),('warped',[[0,-2,0],[12,-2,3],[12,6,6],[0,6,0]])]:
        add(name,c=c);add(name+'_reversed',c=c,reversed_face=True);add(name+'_copies',c=c,copy=True,grouped=True)
    add('on_edge_oblique',points=[[2,-6,-1],[1,-6,-2],[1,-4,-1],[1,-2,0]])
    add('copies',copy=True);add('grouped_move',grouped=True);add('grouped_copies',copy=True,grouped=True)
    for finish in [0,1,2]:add('cancel_'+str(finish),finish=finish,grouped=True)
    add('line',kind='Line',points=[[0,-6,0],[3,-6,0]])
    add('polyline',kind='Polyline')
    add('rotated_cplane',plane=[[0,0,0],[0,0,1],[1,0,0]],points=[[0,-6,0],[0,-6,2],[0,-5,2],[0,-5,3]])
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops)
    from .structural_sdk import validate_operations
    validate_operations(q);return q

def option_request():
    import copy
    q=request();base=q['operations'][0];on=next(op for op in q['operations']if op['id']=='on_edge_oblique')
    rows=[]
    for source in [base,on]:
        for flip,reverse in [(True,False),(False,True),(True,True)]:
            op=copy.deepcopy(source);op['id']+='_bare_'+str(int(flip))+str(int(reverse))
            op['actions'][3]='FlipSurface'if flip else'FlipSurface=No'
            op['actions'][4]='ReverseCurve'if reverse else'ReverseCurve=No'
            rows.append(op)
    return dict(q,operations=rows)

def main():
    from .client import OracleClient
    import sys
    label='orient_curve_edge_options'if sys.argv[1:]==['--options']else'orient_curve_edge'
    q=option_request()if label.endswith('_options')else request();path=HERE/('fixtures/'+label+'.json');path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_orient_curve_edge.py','orient_curve_edge_probe.py','client.py','structural_sdk.py','rhino_worker.py','conic_getter_input.py','number_token.py','join_probe.py','merge_edges_probe.py','managed_mouse_callback.py','snap_environment.py','catenary_point_picker.py','group_picking.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={n:digest(HERE/n)for n in names}
    print('Capturing',len(q['operations']),'curve-to-edge workflows',flush=True)
    r=OracleClient(settings_scheme='VibocerosOracleOrientCurveEdge20261020').run_rhino(q,timeout=1000)
    assert sources=={n:digest(HERE/n)for n in names};r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/('observations/'+label+'.json')).write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    for name,sha in sources.items():
        p=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not p.exists():p.write_bytes(gzip.compress((HERE/name).read_bytes(),mtime=0))
    print('Recorded curve-to-edge workflows',flush=True)
if __name__=='__main__':main()
