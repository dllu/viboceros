"""Closed owned command scenes: output selection, owners and no-op history."""
from copy import deepcopy
STRIP = dict(vertices=[[0,0,0],[2,0,0],[2,2,0],[0,2,0],[4,0,0],[4,2,0]],
             faces=[[0,1,2,3],[1,4,5,2]])
TRI = dict(vertices=[[0,0,0],[2,0,0],[2,2,0],[0,2,0]], faces=[[0,1,2],[0,2,3]])
RING = dict(vertices=[[x,y,0]for y in range(4)for x in range(4)],
            faces=[[4*y+x,4*y+x+1,4*y+x+5,4*y+x+4]for y in range(3)for x in range(3)if(x,y)!=(1,1)])
FACES = [dict(kind='face',index=i)for i in [0,1]]
CASES = {}
for kind in ['mesh','subd']:
    for results in ['Selected','NotSelected']:
        for name,scene,picks in [('faces',STRIP,FACES),('edge',STRIP,[dict(kind='edge',vertices=[1,2])]),
                                ('vertex',TRI,[dict(kind='vertex',index=0)]),
                                ('single',STRIP,[dict(kind='face',index=0)]),
                                ('hole',RING,[dict(kind='face',index=i)for i in range(8)])]:
            CASES[kind+'-'+name+'-'+results] = dict(results=results,
                sources=[dict(kind=kind,picks=picks,**deepcopy(scene))])
for results in ['Selected','NotSelected']:
    sources = [dict(kind=kind,picks=deepcopy(FACES),**deepcopy(STRIP))for kind in ['mesh','subd']]
    for i,source in enumerate(sources):source['vertices']=[[x+9*i,y,z]for x,y,z in source['vertices']]
    CASES['mixed-owners-'+results]=dict(results=results,sources=sources)
    one=deepcopy(sources);one[1]['picks']=[dict(kind='face',index=0)]
    CASES['one-unchanged-owner-'+results]=dict(results=results,sources=one)
    for kind in ['mesh','subd']:
        scene=dict(vertices=STRIP['vertices']+[[6,0,0],[6,2,0]],faces=STRIP['faces']+[[4,6,7,5]])
        CASES[kind+'-ignored-edge-'+results]=dict(results=results,sources=[dict(kind=kind,
            picks=deepcopy(FACES)+[dict(kind='edge',vertices=[4,5])],**scene)])
for kind in ['mesh','subd']:
    CASES[kind+'-whole-owner']=dict(results='Selected',sources=[dict(kind=kind,picks=[dict(kind='owner')],**deepcopy(STRIP))])
CASES['empty'] = dict(results='Selected',sources=[])


EXTRA_CASES = {}
for results in ['Selected','NotSelected']:
    for order in [[0,1],[1,0]]:
        source=deepcopy(CASES['mixed-owners-'+results])
        source['sources'].reverse();source['selection_order']=order
        EXTRA_CASES['reverse-kinds-'+''.join(str(i)for i in order)+'-'+results]=source
    for kind in ['mesh','subd']:
        source=deepcopy(CASES['mixed-owners-'+results])
        for definition in source['sources']:definition['kind']=kind
        source['selection_order']=[1,0]
        EXTRA_CASES[kind+'-reverse-selection-'+results]=source
        source=deepcopy(CASES[kind+'-faces-'+results]);source['postselect']=True
        EXTRA_CASES[kind+'-postselected-'+results]=source
    source=deepcopy(CASES['mesh-faces-'+results]);source['witness_selected']=True
    EXTRA_CASES['wrong-owner-peer-'+results]=source


# Repeat identical public object identities to distinguish insertion ordering
# from UUID ordering. These are private owned documents, cleared after each case.
ORDER_CASES = {}
for kind in ['mesh','subd']:
    for results in ['Selected','NotSelected']:
        for repeat in range(8):
            source=deepcopy(EXTRA_CASES[kind+'-reverse-selection-'+results])
            source['persistent_ids']=['20000000-0000-4000-8000-000000000001',
                                      '10000000-0000-4000-8000-000000000002']
            ORDER_CASES[kind+'-fixed-ids-'+results+'-'+str(repeat)]=source


def validate(op):
    try:
        valid = (isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='merge_faces_command'
                 and isinstance(op['case'],str)and (op['case']in CASES or op['case']in EXTRA_CASES or op['case']in ORDER_CASES) and op['id']==op['case'])
    except TypeError:valid=False
    if not valid:raise ValueError('fixed owned MergeFaces command case required')
