"""Owned face-loop seeds and preselection; held-out cases remain separate."""
def discovery_cases():
    rows=[]
    for kind in ('mesh','subd'):
        for name,pair,initial,steps,flags in [
            ('interior',[6,7],[],[dict(kind='click',modifiers='plain',pair=[6,7])],{}),
            ('boundary',[1,2],[],[dict(kind='click',modifiers='plain',pair=[1,2])],{}),
            ('pre_edge',[6,7],[[6,7]],[],{}),('pre_face',[6,7],[],[],dict(component='face'))]:
            rows.append(dict(dict(op='face_loop_workflow',id=kind+'_'+name,kind=kind,scene='grid',pair=pair,initial=initial,steps=steps,finish='Enter'),**flags))
    return rows

def cases():
    if __package__:
        from .edge_ring_cases import cases as controls,ngon_cases
    else:
        from edge_ring_cases import cases as controls,ngon_cases
    rows=[dict(o,op='face_loop_workflow') for o in controls()+ngon_cases()]
    for kind in ('mesh','subd'):
        for component in ('face','vertex'):
            rows.append(dict(op='face_loop_workflow',id=kind+'_mixed_'+component+'_edge',kind=kind,scene='grid',pair=[6,7],initial=[[6,7]],steps=[],finish='Enter',component=component))
    return rows

def ngon_cases():
    return [dict(op='face_loop_workflow',id=kind+'_ngon_grid_'+str(i),kind=kind,scene='ngon_grid',pair=pair,initial=[],steps=[dict(kind='click',modifiers='plain',pair=pair)],finish='Enter') for kind in ('mesh','subd') for i,pair in enumerate([[2,7],[10,11],[7,12]])]

def validate(op):
    if not isinstance(op,dict) or op not in discovery_cases()+cases()+ngon_cases() or any(type(i) is not int for pair in [op.get('pair',[])]+op.get('initial',[])+[s.get('pair',[]) for s in op.get('steps',[])] for i in pair) or any(k in op and type(op[k]) is not bool for k in ('uv','whole')):
        raise ValueError('fixed owned face-loop workflow required')
