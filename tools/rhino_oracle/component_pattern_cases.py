"""Mixed preselection across all three edge-driven component commands."""
def cases():
    return [dict(op='component_pattern_workflow',id=kind+'_'+scene+'_'+command+'_'+component,kind=kind,scene=scene,pair=[6,7],initial=[[6,7]],steps=[],finish='Enter',component=component,command=command,face_metadata=True)
            for kind in ('mesh','subd') for scene in ('grid','triangles') for command in ('SelEdgeLoop','SelEdgeRing','SelFaceLoop') for component in ('face','vertex')]
def nonmanifold_cases():
    return [dict(op='component_pattern_workflow',id='mesh_nonmanifold_'+command+'_'+component,kind='mesh',scene='nonmanifold',pair=[6,7],initial=[[6,7]],steps=[],finish='Enter',component=component,command=command,face_metadata=True) for command in ('SelEdgeLoop','SelEdgeRing','SelFaceLoop') for component in ('face','vertex')]
def validate(op):
    if not isinstance(op,dict) or op not in cases()+nonmanifold_cases() or type(op.get('face_metadata')) is not bool or any(type(i) is not int for pair in [op.get('pair',[])]+op.get('initial',[]) for i in pair):
        raise ValueError('fixed owned component-pattern workflow required')
