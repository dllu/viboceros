"""Owned polygon edges for Length, including extraordinary and curved SubD nets."""
if __package__:
    from .edge_loop_cases import source as loop_source
else:
    from edge_loop_cases import source as loop_source

def source(scene):
    if scene=='surface':return dict(points=[[0.,0.,0.],[3.,0.,0.],[3.,3.,0.],[0.,3.,0.]],faces=[[0,1,2,3]])
    if scene=='cube':
        return dict(points=[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.],
            [0.,0.,5.],[4.,0.,5.],[4.,3.,5.],[0.,3.,5.]],
            faces=[[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]])
    name='fan5' if scene.startswith('fan') else 'grid'
    result=loop_source(name)
    if scene in ('bent_grid','creased_grid'):
        result['points']=[[x,y,.04*x*x+.07*y*y+.03*x*y]for x,y,z in result['points']]
    if scene=='creased_grid':result['creases']=[[i,i+1]for i in range(5,9)]
    if scene=='raised_fan':
        result=loop_source('fan5')
        result['points']=[[x,y,2. if i==0 else .05*x*y]for i,(x,y,z)in enumerate(result['points'])]
    if scene not in ('grid','bent_grid','creased_grid','fan5','raised_fan'):raise ValueError('owned edge length scene required')
    return result

def cases():
    rows=[]
    for kind in ('mesh','subd'):
        for scene,pairs in [('grid',[[0,1]]),('bent_grid',[[6,7]]),('bent_grid',[[1,2]]),
            ('creased_grid',[[6,7]]),('fan5',[[0,1]]),('raised_fan',[[0,1]]),('cube',[[0,1]])]:
            for flat in ([False] if kind=='mesh' else [False,True]):
                rows.append(dict(op='component_length',id=kind+'_'+scene+'_'+str(pairs[0][0])+'_'+str(int(flat)),kind=kind,scene=scene,pairs=pairs,flat=flat,curve=False))
        rows.append(dict(op='component_length',id=kind+'_mixed',kind=kind,scene='bent_grid',pairs=[[6,7],[7,8]],flat=False,curve=True))
    return rows

def validate(op):
    if not isinstance(op,dict) or op not in cases()+followups()+workflows() or type(op.get('flat')) is not bool or ('bare' in op and type(op['bare'])is not bool) or type(op.get('curve')) is not bool or any(type(v)is not int for pair in op.get('pairs',[])for v in pair):
        raise ValueError('fixed owned edge length case required')


def followups():
    return [dict(op='component_length',id='bare_'+kind+'_'+str(int(flat))+'_'+str(int(curve)),kind=kind,scene='surface' if kind=='brep' else 'bent_grid',pairs=[[0,1]] if kind=='brep' else [[6,7]],flat=flat,curve=curve,bare=True) for kind,flat,curve in [('mesh',False,False),('subd',False,False),('subd',True,False),('brep',False,False),('brep',False,True),('subd',False,True),('mesh',False,True)]]


def workflows():
    rows=[]
    for kind in ('mesh','subd','brep'):
        for flat in ([False,True] if kind=='subd' else [False]):
            for finish in ('Enter','Cancel'):
                rows.append(dict(op='component_length',id='picked_'+kind+'_'+str(int(flat))+'_'+finish,kind=kind,scene='surface' if kind=='brep' else 'bent_grid',pairs=[[0,1]] if kind=='brep' else [[6,7]],flat=flat,curve=False,steps=[dict(kind='click',modifiers='sub')],finish=finish))
    for kind in ('mesh','subd'):
        rows.append(dict(op='component_length',id='picked_mixed_'+kind,kind=kind,scene='bent_grid',pairs=[[6,7]],flat=False,curve=True,steps=[dict(kind='click',modifiers='sub'),dict(kind='click',modifiers='plain',target='curve')],finish='Enter'))
        rows.append(dict(op='component_length',id='picked_remove_'+kind,kind=kind,scene='bent_grid',pairs=[[6,7]],flat=False,curve=False,steps=[dict(kind='click',modifiers='sub'),dict(kind='click',modifiers='ctrl')],finish='Cancel'))
    return rows
