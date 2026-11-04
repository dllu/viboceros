"""Owned edge-ring recipes; discovery stays separate from topology holdouts."""
def discovery_cases():
    return [dict(op='edge_ring_workflow',id=kind+'_interior',kind=kind,scene='grid',pair=[6,7],initial=[],
                 steps=[dict(kind='click',modifiers='plain',pair=[6,7])],finish='Enter') for kind in ('mesh','subd')]

def cases():
    if __package__:
        from .edge_loop_cases import cases as topology
        from .edge_loop_workflow_probe import discovery_cases as entry,cases as workflows
    else:
        from edge_loop_cases import cases as topology
        from edge_loop_workflow_probe import discovery_cases as entry,cases as workflows
    rows=[dict(o,op='edge_ring_workflow',steps=[dict(kind='click',modifiers='plain',pair=o['pair'])],initial=[],finish='Enter') for o in topology()]
    rows.extend(dict(o,op='edge_ring_workflow',id='workflow_'+o['id']) for o in entry()+workflows())
    for scene,pairs in [('nonmanifold',[[6,7],[1,2]]),('quad_triangle',[[6,7],[1,2]]),('unwelded',[[6,7]])]:
        rows.extend(dict(op='edge_ring_workflow',id='mesh_'+scene+'_'+str(i),kind='mesh',scene=scene,pair=pair,initial=[],steps=[dict(kind='click',modifiers='plain',pair=pair)],finish='Enter') for i,pair in enumerate(pairs))
    return rows

def ngon_cases():
    return [dict(op='edge_ring_workflow',id=kind+'_'+scene,kind=kind,scene=scene,pair=pair,initial=[],steps=[dict(kind='click',modifiers='plain',pair=pair)],finish='Enter') for kind in ('mesh','subd') for scene,pair in [('ngon_strip',[0,3]),('ngon_pentagon',[0,1]),('ngon_quad',[0,1]),('ngon_pair',[6,7])]]

def source(scene):
    if scene.startswith('ngon_'):
        if scene=='ngon_pentagon':
            import math
            return dict(points=[[4.*math.cos(2.*math.pi*i/5),4.*math.sin(2.*math.pi*i/5),0.] for i in range(5)],faces=[[0,1,2],[0,2,3],[0,3,4]],ngons=[dict(vertices=list(range(5)),faces=[0,1,2])])
        if scene=='ngon_quad':return dict(points=[[0.,0.,0.],[3.,0.,0.],[3.,3.,0.],[0.,3.,0.]],faces=[[0,1,2],[0,2,3]],ngons=[dict(vertices=[0,1,2,3],faces=[0,1])])
        r=dict(points=[[0.,0.,0.],[3.,0.,0.],[6.,0.,0.],[0.,3.,0.],[3.,3.,0.],[6.,3.,0.]],faces=[[0,1,4,3],[1,2,5,4]],ngons=[dict(vertices=[0,1,2,5,4,3],faces=[0,1])])
        if scene=='ngon_pair':r['points'] += [[9.,0.,0.],[9.,3.,0.]];r['faces'].append([2,6,7,5])
        if scene not in ('ngon_strip','ngon_pair'):raise ValueError('unknown owned n-gon')
        return r

    if __package__:
        from .edge_loop_cases import source as base
    else:
        from edge_loop_cases import source as base
    if scene not in ('nonmanifold','quad_triangle','collapsed','unwelded'):return base(scene)
    r=base('grid')
    if scene=='nonmanifold':
        r['points'] += [[4.5,4.,3.],[4.5,2.,3.]];r['faces'].append([6,7,20,21])
    elif scene=='quad_triangle':r['faces'][5:6]=[[6,7,12],[6,12,11]]
    elif scene=='collapsed':r['points'][7]=r['points'][6][:]
    elif scene=='unwelded':
        old=r['faces'];r['faces']=[]
        for face in old:
            start=len(r['points']);r['points'].extend(r['points'][i][:] for i in face);r['faces'].append(list(range(start,start+4)))
    return r

def validate(op):
    if not isinstance(op,dict) or op not in discovery_cases()+cases()+ngon_cases() or any(type(i) is not int for pair in [op.get('pair',[])]+op.get('initial',[])+[s.get('pair',[]) for s in op.get('steps',[])] for i in pair) or any(k in op and type(op[k]) is not bool for k in ('uv','whole')):
        raise ValueError('fixed owned edge-ring workflow required')
