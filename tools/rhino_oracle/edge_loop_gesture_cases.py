"""Fixed owned idle loop-gesture recipes."""
def cases():
    rows=[]
    for kind in ('mesh','subd'):
        for name,scene,pair,initial in [
            ('add','grid',[6,7],[]),
            ('remove','grid',[6,7],[[5,6],[6,7],[7,8],[8,9]]),
            ('range','grid',[7,8],[[6,7],[8,9]]),
            ('fragment','grid',[6,7],[[5,6],[6,7],[8,9]]),
            ('one_anchor','grid',[7,8],[[6,7]]),
            ('single_remove','grid',[6,7],[[6,7]]),
            ('closed_range','annulus',[9,10],[[8,9],[12,13]]),
            ('closed_fragment','annulus',[8,9],[[15,8],[8,9],[9,10],[11,12],[12,13],[13,14]])]:
            rows.append(dict(op='edge_loop_gesture',id=kind+'_'+name,kind=kind,scene=scene,pair=pair,initial=initial))
    return rows

def validate(op):
    if not isinstance(op,dict) or op not in cases()+filtered_cases()+subobject_cases()+modified_cases() or any(type(i) is not int for pair in [op.get('pair',[])]+op.get('initial',[]) for i in pair):raise ValueError('fixed owned edge-loop gesture required')

def filtered_cases():
    return [dict(op,op='edge_loop_filtered_gesture') for op in cases()]

def subobject_cases():
    return [dict(op,op='edge_loop_subobject_gesture') for op in cases()]

def modified_cases():
    return [dict(op,op='edge_loop_modified_gesture',modifiers='ctrl' if op['id'].endswith(('remove','fragment')) else 'shift') for op in cases()]
