"""Fixed edge-spin geometry and command-local history observations."""
from copy import deepcopy
try:
    from .subdivide_probe import source,STRIP,CUBE,CUBE_FACES
except (ImportError,ValueError):
    from subdivide_probe import source,STRIP,CUBE,CUBE_FACES

def strip(**values):return source(vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],edges=[[1,4]],**values)
def cube(**values):return source(vertices=CUBE,faces=CUBE_FACES,edges=[[0,1]],**values)

DISCOVERY={}
for clockwise in [False,True]:
    for shape,definition in [('strip',strip()),('cube',cube())]:
        name='spin_'+shape+('_clockwise'if clockwise else'_counterclockwise')
        DISCOVERY[name]=dict(command='SubDSpinEdge',sources=[definition],prime_arguments='_Direction=_'+('Clockwise'if clockwise else'Counter-clockwise'))
CASES=dict(DISCOVERY)
for shape,definition in [('strip_crease',strip(creases=[[1,4]])),('strip_corner',strip(creases=[[1,4]],corners=[1])),('cube_soft',cube(soft=50)),('boundary',source(edges=[[0,1]])),('panel',source(vertices=[[float(x),float(y),.2*((x+y)%2)]for y in range(3)for x in range(3)],faces=[[0,1,4,3],[1,2,5,4],[3,4,7,6],[4,5,8,7]],edges=[[1,4]]))]:
    for clockwise in [False,True]:
        name='spin_'+shape+('_clockwise'if clockwise else'_counterclockwise')
        CASES[name]=dict(command='SubDSpinEdge',sources=[definition],prime_arguments='_Direction=_'+('Clockwise'if clockwise else'Counter-clockwise'))
for name,arguments,cancel in [('again','_SpinEdgeAgain',False),('twice_again','_SpinEdgeAgain _SpinEdgeAgain',False),('undo','_Undo',False),('again_undo','_SpinEdgeAgain _Undo',False),('cancel','',True)]:
    CASES['spin_strip_'+name]=dict(command='SubDSpinEdge',sources=[strip()],prime_arguments='_Direction=_Clockwise',command_arguments=arguments,cancel=cancel)
LIVE={}
for direction in ['Clockwise','CounterClockwise']:
    for name,tail in [('once',['Enter']),('again',['SpinEdgeAgain','Enter']),('twice_again',['SpinEdgeAgain','SpinEdgeAgain','Enter']),('undo',['Undo','Enter']),('again_undo',['SpinEdgeAgain','Undo','Enter']),('cancel',['Cancel'])]:
        definition=source(vertices=[[0.,0.,0.],[2.,0.,0.],[4.,0.,0.],[0.,3.,0.],[2.,3.,0.],[4.,3.,0.]],faces=[[0,1,4,3],[1,2,5,4]],edges=[])
        LIVE['spin_live_'+direction.lower()+'_'+name]=dict(command='SubDSpinEdge',sources=[definition],live=True,actions=['Direction',direction,{'click':[2.,1.5,0.]}]+tail)
LIVE_V2={}
for key,recipe in LIVE.items():
    row=deepcopy(recipe);row['actions']=['Counter-clockwise'if a=='CounterClockwise'else a for a in row['actions']]
    LIVE_V2[key.replace('spin_live_','spin_live_v2_')]=row
LIVE_V3={}
for key,recipe in LIVE_V2.items():
    row=deepcopy(recipe);clockwise='_clockwise_'in key
    row['actions']=(['Direction']if clockwise else[])+row['actions'][2:]
    LIVE_V3[key.replace('v2_','v3_')]=row
base=deepcopy(next(iter(LIVE_V3.values())))
for name,actions in [('switch',['Direction',{'click':[2.,1.5,0.]},'Direction','SpinEdgeAgain','Enter']),('undo_again',[{'click':[2.,1.5,0.]},'SpinEdgeAgain','Undo','SpinEdgeAgain','Enter']),('undo_all_again',[{'click':[2.,1.5,0.]},'Undo','SpinEdgeAgain','Enter'])]:
    row=deepcopy(base);row['actions']=actions;LIVE_V3['spin_live_v3_'+name]=row
HOLDOUTS={}
for name,shapes in [('two_objects',[strip(),source(vertices=[[x+10,y,z]for x,y,z in STRIP],faces=[[0,1,4,3],[1,2,5,4]],edges=[[1,4]])]),('multiple_edges',[source(vertices=[[float(x),float(y),0.]for y in range(3)for x in range(3)],faces=[[0,1,4,3],[1,2,5,4],[3,4,7,6],[4,5,8,7]],edges=[[1,4],[3,4]])]),('boundary_and_interior',[source(vertices=STRIP,faces=[[0,1,4,3],[1,2,5,4]],edges=[[0,1],[1,4]])]),('triangles',[source(vertices=[[0.,0.,0.],[3.,0.,0.],[3.,2.,0.],[0.,2.,0.]],faces=[[0,1,2],[0,2,3]],edges=[[0,2]])])]:
    HOLDOUTS['spin_holdout_'+name]=dict(command='SubDSpinEdge',sources=shapes)
LIVE_V4={}
flat=deepcopy(base['sources'][0]);flat['edges']=[]
second=deepcopy(flat);second['vertices']=[[x+10,y,z]for x,y,z in flat['vertices']]
for name,tail in [('two_objects',['Enter']),('undo_second',['Undo','Enter']),('undo_both',['Undo','Undo','Enter']),('revisit_first',[{'click':[2.,1.5,0.],'source':0},'Enter'])]:
    LIVE_V4['spin_live_v4_'+name]=dict(command='SubDSpinEdge',sources=[deepcopy(flat),deepcopy(second)],live=True,actions=[{'click':[2.,1.5,0.],'source':0},{'click':[12.,1.5,0.],'source':1}]+tail)
for selected in [False,True]:
    shape=deepcopy(flat);shape['selected_grips']=[0]if selected else[]
    LIVE_V4['spin_live_v4_grips'+('_selected'if selected else'')]=dict(command='SubDSpinEdge',sources=[shape],live=True,grips=True,actions=[{'click':[2.,1.5,0.]},'Enter'])
LIVE_V5={k.replace('v4_','v5_'):deepcopy(v)for k,v in LIVE_V4.items()if not v.get('grips')}
ALL_CASES=dict(CASES,**dict(LIVE,**dict(LIVE_V2,**dict(LIVE_V3,**dict(HOLDOUTS,**dict(LIVE_V4,**LIVE_V5))))))
def request(discovery=False,live=False,live_v2=False,live_v3=False,holdouts=False,live_v4=False,live_v5=False):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_spin_edge',id=k,case=k)for k in (LIVE_V5 if live_v5 else LIVE_V4 if live_v4 else HOLDOUTS if holdouts else LIVE_V3 if live_v3 else LIVE_V2 if live_v2 else LIVE if live else DISCOVERY if discovery else CASES)])
def validate(op):
    try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_spin_edge'and op['case']in ALL_CASES and op['id']==op['case']
    except TypeError:valid=False
    if not valid:raise ValueError('fixed SubD edge spin recipe required')
