"""Authored open rectangular walls; no observed geometry is used as input."""
from copy import deepcopy


def walls(shape, scale=1., reverse=False):
    rings = [[[0.,0.],[1.,0.],[1.,1.],[0.,1.]]]
    if shape == 'annulus':rings.append([[.25,.25],[.75,.25],[.75,.75],[.25,.75]])
    if shape in ('separated','narrow'):
        gap = 2.**-43 if shape == 'narrow' else .125
        rings.extend([[[.1,.2],[.5,.2],[.5,.8],[.1,.8]],
                      [[.5+gap,.2],[.9,.2],[.9,.8],[.5+gap,.8]]])
    vertices=[];faces=[]
    for index,ring in enumerate(rings):
        base=len(vertices)
        for z in [0.,2.]:vertices.extend([[scale*x,scale*y,z]for x,y in ring])
        for side in range(4):
            nxt=(side+1)%4
            f=[base+side,base+nxt,base+nxt+4,base+side+4]if index==0 else[base+side,base+side+4,base+nxt+4,base+nxt]
            faces.append(list(reversed(f))if reverse else f)
    return dict(kind='mesh',vertices=vertices,faces=faces)


CASES={}
def add(name,shape,scale=1.,reverse=False,delete=True,crease=True,triangles=True):
    CASES[name]=dict(command='Cap',sources=[walls(shape,scale,reverse)],whole=True,preselected=True,
        mesh_channels=False,command_arguments=' '.join('_'+key+'=_'+('Yes'if value else'No')for key,value in [('DeleteInput',delete),('Crease',crease),('Triangles',triangles)]))

for shape in ['single','annulus','separated','narrow']:
    add(shape,shape)
for shape in ['separated','narrow']:add(shape+'-large',shape,1e8)
add('narrow-reversed','narrow',reverse=True)
add('separated-large-reversed','separated',1e8,reverse=True)
for shape,scale in [('separated',1.),('narrow',1e8)]:
    add(shape+'-welded',shape,scale,crease=False)
    add(shape+'-copy',shape,scale,delete=False)
for shape in ['single','annulus']:add(shape+'-ngon',shape,triangles=False)


REGIONS=tuple(CASES)
GROUP_CASES=[]
for name,shape,crease,triangles,partial,closed in [
    ('copy-pair','separated',True,True,False,False),
    ('copy-partial-group','separated',True,True,True,False),
    ('copy-mixed-regions','annulus',True,True,False,False),
    ('copy-closed-peer','single',True,True,False,True),
    ('copy-welded-pair','separated',False,True,False,False),
    ('copy-ngon-pair','single',True,False,False,False),
]:
    add(name,shape,delete=False,crease=crease,triangles=triangles)
    case=CASES[name]
    second=walls('separated'if shape=='annulus'else shape)
    second['vertices']=[[x+2.,y,z]for x,y,z in second['vertices']]
    case['sources'].append(second)
    if partial:case['selected_sources']=[0]
    if closed:case['sources'][0]['faces'].extend([[0,3,2,1],[4,5,6,7]])
    GROUP_CASES.append(name)


NOOP_CASES=[]
for name,pair in [('closed-noop',False),('closed-pair-noop',True)]:
    add(name,'single')
    case=CASES[name];case.pop('command_arguments')
    case['sources'][0]['faces'].extend([[0,3,2,1],[4,5,6,7]])
    if pair:
        second=deepcopy(case['sources'][0]);second['vertices']=[[x+2.,y,z]for x,y,z in second['vertices']]
        case['sources'].append(second)
    NOOP_CASES.append(name)


def validate(op):
    if not(isinstance(op,dict)and set(op)=={'op','id','case'}and op.get('op')=='mesh_cap_regions'
           and isinstance(op.get('case'),str)and op['case']in CASES and op['id']==op['case']):
        raise ValueError('fixed owned mesh cap region recipe required')
