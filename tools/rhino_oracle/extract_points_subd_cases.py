"""Whole SubDs in smooth and flat appearance, with independent selection paths."""
from copy import deepcopy

WHOLE = {}
for kind in ['subd','subd_irregular','subd_grid','subd_strip','subd_triangles','subd_creased']:
    for flat in [False,True]:
        for output in ['Points','PointCloud']:
            name='whole_'+kind+('_flat_' if flat else '_smooth_')+output.lower()
            WHOLE[name]=dict(sources=[dict(kind=kind,mode=None,grips=[])],
                whole=[0],order=[0],output=output,layer='Input',flat=flat,whole_subd=True)

PICKED={'picked_'+name:dict(deepcopy(case),getter=True) for name,case in WHOLE.items()}

MIXED = {}
for picked in [False,True]:
    for output in ['Points','PointCloud']:
        for grips in [False,True]:
            name='whole_subd_mixed_'+('picked_' if picked else 'preselected_')+output.lower()+('_grips' if grips else '_reverse')
            MIXED[name]=dict(sources=[dict(kind='subd_irregular',mode=None,grips=[]),
                dict(kind='circle',mode='control' if grips else None,grips=[0,2] if grips else [])],
                whole=[0] if grips else [1,0],order=[1,0],output=output,layer='Current' if grips else 'Input',
                flat=False,whole_subd=True,getter=picked)

VERIFIED = {'verified_'+name:dict(deepcopy(case),whole_subd_configured=not case.get('getter',False))
    for name,case in list(WHOLE.items())+list(MIXED.items())}

# Preselection consumes the first option and executes immediately. Cloud output
# therefore uses Output first and Rhino's default Current output layer. The
# command-first scenes independently exercise input-layer clouds.
FINAL = {}
for name,case in list(WHOLE.items())+list(MIXED.items()):
    case=deepcopy(case)
    if not case.get('getter') and case['output']=='PointCloud':
        case.update(layer='Current',output_first=True)
    FINAL['final_'+name]=case
