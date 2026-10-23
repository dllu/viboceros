"""Independent physical face picks for native triangulation threshold getters."""
from copy import deepcopy
WARP=[[0.,0.,0.],[2.,0.,0.],[2.,1.,0.],[0.,1.,.4]]
OPTIONS='Change option or press Enter to accept'
FACE='Select mesh face'


def scene(name,angle=False,switch=False,accept='',shaded=False,vertices=None,pick=None):
    # Distinct numeric witnesses reveal whether a face pick changes both values.
    actions=['SelectBy=Distance','Distance',.123,'SelectBy=Angle','Angle',7.,'SelectBy='+('Angle'if angle else'Distance'),'Angle'if angle else'Distance',dict(click=[.8,.6,.2]if pick is None else pick)]
    prompts=[OPTIONS,OPTIONS,FACE,OPTIONS,OPTIONS,FACE,OPTIONS,OPTIONS,FACE]
    if switch:
        actions.append('SelectBy='+('Distance'if angle else'Angle'));prompts.append(OPTIONS)
    actions.append(accept);prompts.append(OPTIONS)
    result=dict(command='TriangulateNonPlanarQuads',whole=True,preselected=True,live=True,capture_id='nonplanar_sample_'+name,
        sources=[dict(kind='mesh',vertices=deepcopy(WARP if vertices is None else vertices),faces=[[0,1,2,3]])],
        actions=actions,action_prompts=[[p]for p in prompts],sample_face=0,sample_angle=angle,switch=switch)
    if shaded:result['sample_shaded']=True
    return result


DISCOVERY={}
for name,angle,switch in [('distance',False,False),('distance_switch',False,True),('angle',True,False),('angle_switch',True,True)]:
    DISCOVERY['nonplanar_sample_'+name]=scene(name,angle,switch)
SHADED={}
for name,angle in [('shaded_distance',False),('shaded_angle',True)]:
    SHADED['nonplanar_sample_'+name]=scene(name,angle,accept='Enter',shaded=True)
MAIN={}
def main(name,**kw):
    MAIN['nonplanar_sample_'+name]=scene(name,accept='Enter',shaded=True,**kw)
main('distance_switch_main',switch=True)
main('angle_switch_main',angle=True,switch=True)
main('cyclic_distance')
MAIN['nonplanar_sample_cyclic_distance']['sources'][0]['faces']=[[1,2,3,0]]
main('reverse_angle',angle=True)
MAIN['nonplanar_sample_reverse_angle']['sources'][0]['faces']=[[0,3,2,1]]
for name,scale in [('scaled_large',10.),('scaled_small',.001)]:
    main(name,vertices=[[c*scale for c in p]for p in WARP],pick=[.8*scale,.6*scale,.2*scale])
for name,tolerance in [('tolerance_low',1e-6),('tolerance_high',.1)]:
    main(name);MAIN['nonplanar_sample_'+name]['sample_tolerance']=tolerance
main('angular_tolerance',angle=True)
MAIN['nonplanar_sample_angular_tolerance']['sample_angle_tolerance']=.5
flat=[[x,y,0.]for x,y,z in WARP]
main('flat_distance',vertices=flat)
main('flat_angle',angle=True,vertices=flat)
main('ngon_second',vertices=[[0.,0.,0.],[2.,0.,0.],[4.,0.,0.],[0.,1.,.4],[2.,1.,.4],[4.,1.,0.]],pick=[3.,.5,.1])
MAIN['nonplanar_sample_ngon_second']['sample_face']=1
MAIN['nonplanar_sample_ngon_second']['sources'][0].update(faces=[[0,1,4,3],[1,2,5,4]],ngons=[dict(vertices=[0,1,2,5,4,3],faces=[0,1])])
main('external_reference',vertices=[[x,y,2*z]for x,y,z in WARP],pick=[x+3. if i==0 else x for i,x in enumerate([.8,.6,.2])])
MAIN['nonplanar_sample_external_reference']['sources'].append(dict(kind='mesh',vertices=[[x+3.,y,z]for x,y,z in WARP],faces=[[0,1,2,3]]))
MAIN['nonplanar_sample_external_reference'].update(selected_sources=[0],sample_source=1)
main('cancel_face')
row=MAIN['nonplanar_sample_cancel_face'];row['actions']=row['actions'][:8]+['Cancel','Cancel'];row['action_prompts']=row['action_prompts'][:8]+[[FACE],[OPTIONS]];row['sample_cancel']=True
MAIN['nonplanar_sample_after_cancel']=dict(command='TriangulateNonPlanarQuads',whole=True,preselected=True,sources=[dict(kind='mesh',vertices=deepcopy(WARP),faces=[[0,1,2,3]])],command_arguments='',actions=[])
main('enter_keeps')
row=MAIN['nonplanar_sample_enter_keeps'];row['actions'][8]='Enter';row['sample_keep']=True
CASES=dict(DISCOVERY);CASES.update(SHADED);CASES.update(MAIN)
CORE=dict((k,v)for k,v in MAIN.items()if k not in ('nonplanar_sample_ngon_second','nonplanar_sample_external_reference'))
RULES={}
for name,height in [('power_one',.5),('power_tenth',.05)]:
    RULES['nonplanar_sample_'+name]=scene(name,accept='Enter',shaded=True,vertices=[[-1.,-1.,height],[1.,-1.,-height],[1.,1.,height],[-1.,1.,-height]],pick=[0.,0.,0.])
RULES['nonplanar_sample_small_angle']=scene('small_angle',angle=True,accept='Enter',shaded=True,vertices=[[x,y,z*.00025]for x,y,z in WARP])
for name,mode,option in [('negative_distance','Distance','Distance'),('negative_angle','Angle','Angle')]:
    RULES['nonplanar_sample_'+name]=dict(command='TriangulateNonPlanarQuads',whole=True,preselected=True,sources=[dict(kind='mesh',vertices=deepcopy(flat),faces=[[0,1,2,3]])],command_arguments='_SelectBy=_'+mode+' _'+option+' -0.01',actions=[])
CASES.update(RULES)
TOPOLOGY={}
for name in ['raw_second','quad_ngon']:
    row=deepcopy(MAIN['nonplanar_sample_ngon_second']);row['capture_id']='nonplanar_sample_'+name
    row['sources'][0].pop('ngons')
    if name=='quad_ngon':row['sources'][0]['ngons']=[dict(vertices=[1,2,5,4],faces=[1])]
    TOPOLOGY['nonplanar_sample_'+name]=row
CASES.update(TOPOLOGY)
NEIGHBORS={}
for name,height in [('power_below',.49999999999999994),('power_above',.5000000000000001)]:
    NEIGHBORS['nonplanar_sample_'+name]=scene(name,accept='Enter',shaded=True,vertices=[[-1.,-1.,height],[1.,-1.,-height],[1.,1.,height],[-1.,1.,-height]],pick=[0.,0.,0.])
CASES.update(NEIGHBORS)


def request(shaded=False,main=False,core=False,rules=False,external=False,raw=False,quad_ngon=False,neighbors=False):
    families=[(shaded,SHADED),(main,MAIN),(core,CORE),(rules,RULES),(neighbors,NEIGHBORS),
        (external,dict((k,v)for k,v in MAIN.items()if k=='nonplanar_sample_external_reference')),
        (raw,dict((k,v)for k,v in TOPOLOGY.items()if k=='nonplanar_sample_raw_second')),
        (quad_ngon,dict((k,v)for k,v in TOPOLOGY.items()if k=='nonplanar_sample_quad_ngon'))]
    selected=[v for enabled,v in families if enabled]
    if len(selected)>1:raise ValueError('choose one face-sampling family')
    family=selected[0]if selected else DISCOVERY
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),
        operations=[dict(op='nonplanar_quad_sample',id=k,case=k)for k in family])


def validate(op):
    try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='nonplanar_quad_sample'and op['case']in CASES and op['id']==op['case']
    except TypeError:valid=False
    if not valid:raise ValueError('fixed nonplanar face-sampling recipe required')
