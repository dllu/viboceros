"""Fixed public SubD truncated-cone inputs in an owned idle document."""
import math

def scene(base,height,end,around=8,vertical=2,cap='Quad',solid=True,direction='Vertical',**options):
 base=list(base)
 if isinstance(base[0],list)and type(base[1])in(int,float):base[1:2]=['Circumference',2.*math.pi*base[1]]
 return dict(actions=['Solid=Yes','AroundFaces=8','CapFaceStyle='+cap,'AroundFaces='+str(around),'VerticalFaces='+str(vertical),'Solid='+('Yes'if solid else'No')]+base+['DirectionConstraint='+direction,height]+(end if isinstance(end,list)and end and isinstance(end[0],str)else[end]),**options)
DISCOVERY={
 'truncated_discovery_default':dict(actions=[[0.,0.,0.],'Enter','Enter','Enter']),
 'truncated_discovery_quad':scene([[0.,0.,0.],3.],5.,1.),
 'truncated_discovery_tri':scene([[0.,0.,0.],3.],5.,1.,cap='Tri'),
 'truncated_discovery_open':scene([[0.,0.,0.],3.],5.,1.,solid=False),
 'truncated_discovery_three':scene([[0.,0.,0.],3.],5.,1.,around=3,cap='Tri'),
 'truncated_discovery_four_quad':scene([[0.,0.,0.],3.],5.,1.,around=4),
 'truncated_discovery_four_tri':scene([[0.,0.,0.],3.],5.,1.,around=4,cap='Tri'),
 'truncated_discovery_larger':scene([[0.,0.,0.],3.],5.,5.),
 'truncated_discovery_equal':scene([[0.,0.,0.],3.],5.,3.),
 'truncated_discovery_negative':scene([[1.,2.,3.],2.],-5.,1.),
 'truncated_discovery_free':scene([[0.,0.,0.],3.],[3.,4.,6.],1.,direction='None'),
 'truncated_discovery_end_point':scene([[0.,0.,0.],3.],5.,[2.,3.,9.]),
 'truncated_discovery_end_diameter':scene([[0.,0.,0.],3.],5.,['Diameter',2.]),
 'truncated_discovery_end_zero':scene([[0.,0.,0.],3.],5.,0.),
 'truncated_discovery_one_band':scene([[0.,0.,0.],3.],5.,1.,vertical=1),
 'truncated_discovery_both':scene([[0.,0.,0.],3.],5.,1.),
 'truncated_discovery_base_angle':scene([[0.,0.,0.],3.],'OrBaseAngle=30',1.),
 'truncated_discovery_end_area':scene([[0.,0.,0.],3.],5.,['Area',1.]),
}
DISCOVERY['truncated_discovery_both']['actions'].insert(-2,'BothSides=Yes')
def native_scene(base,height,end,**kw):
 case=scene(base,height,end,**kw);actions=case['actions'];i=next(i for i,a in enumerate(actions)if isinstance(a,str)and a.startswith('DirectionConstraint='));direction=actions.pop(i);actions.insert(0,direction)
 # Explicit final mode prevents unrelated cached diameter settings changing a recipe.
 if not(isinstance(end,list)and end and isinstance(end[0],str)):actions.insert(len(actions)-1,'Radius')
 return case
SCENES={
 'subd_truncated_cone_cancel_center':dict(actions=['Cancel'],cancel=True),
 'subd_truncated_cone_default':dict(actions=[[0.,0.,0.],'Enter','Enter','Enter']),
 'subd_truncated_cone_quad':native_scene([[0.,0.,0.],3.],5.,1.),
 'subd_truncated_cone_tri':native_scene([[0.,0.,0.],3.],5.,1.,cap='Tri'),
 'subd_truncated_cone_open':native_scene([[0.,0.,0.],3.],5.,1.,solid=False),
 'subd_truncated_cone_three':native_scene([[0.,0.,0.],3.],5.,1.,around=3,cap='Tri'),
 'subd_truncated_cone_four_quad':native_scene([[0.,0.,0.],3.],5.,1.,around=4),
 'subd_truncated_cone_four_tri':native_scene([[0.,0.,0.],3.],5.,1.,around=4,cap='Tri'),
 'subd_truncated_cone_odd':native_scene([[0.,0.,0.],3.],5.,1.,around=7),
 'subd_truncated_cone_larger':native_scene([[0.,0.,0.],3.],5.,5.),
 'subd_truncated_cone_equal':native_scene([[0.,0.,0.],3.],5.,3.),
 'subd_truncated_cone_zero':native_scene([[0.,0.,0.],3.],5.,0.),
 'subd_truncated_cone_zero_open':native_scene([[0.,0.,0.],3.],5.,0.,solid=False),
 'subd_truncated_cone_negative':native_scene([[1.,2.,3.],2.],-5.,1.),
 'subd_truncated_cone_free':native_scene([[0.,0.,0.],3.],[3.,4.,6.],1.,direction='None'),
 'subd_truncated_cone_free_odd':native_scene([[1.,2.,3.],2.],[-2.,-2.,-3.],4.,direction='None',around=7,solid=False),
 'subd_truncated_cone_height_point':native_scene([[0.,0.,0.],3.],[3.,4.,6.],1.),
 'subd_truncated_cone_radius_point':native_scene([[1.,2.,3.],[4.,6.,15.]],5.,1.),
 'subd_truncated_cone_diameter_base':native_scene([[0.,0.,0.],'Diameter',6.],5.,1.),
 'subd_truncated_cone_diameter_end':native_scene([[0.,0.,0.],3.],5.,['Diameter',2.]),
 'subd_truncated_cone_diameter_point':native_scene([[0.,0.,0.],3.],5.,['Diameter',[2.,3.,9.]]),
 'subd_truncated_cone_end_point':native_scene([[0.,0.,0.],3.],5.,[2.,3.,5.]),
 'subd_truncated_cone_end_spatial':native_scene([[0.,0.,0.],3.],5.,[2.,3.,9.]),
 'subd_truncated_cone_circumference':native_scene([[0.,0.,0.],'Circumference',6.*math.pi],5.,1.),
 'subd_truncated_cone_area':native_scene([[0.,0.,0.],'Area',9.*math.pi],5.,1.),
 'subd_truncated_cone_two_point':native_scene(['2Point',[-3.,0.,0.],[3.,0.,0.]],5.,1.),
 'subd_truncated_cone_three_point':native_scene(['3Point',[3.,0.,0.],[0.,3.,0.],[-3.,0.,0.]],5.,1.),
 'subd_truncated_cone_spatial_circle':native_scene(['3Point',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]],4.,1.),
 'subd_truncated_cone_tilted':native_scene([[10.,20.,30.],3.],5.,1.,construction_plane=[[10.,20.,30.],[.6,.8,0.],[0.,0.,1.]]),
 'subd_truncated_cone_three_bands':native_scene([[0.,0.,0.],3.],5.,1.,vertical=3),
 'subd_truncated_cone_cancel_radius':dict(actions=[[0.,0.,0.],'Cancel'],cancel=True),
 'subd_truncated_cone_cancel_height':dict(actions=[[0.,0.,0.],3.,'Cancel'],cancel=True),
 'subd_truncated_cone_cancel_end':dict(actions=[[0.,0.,0.],3.,5.,'Cancel'],cancel=True),
}

# Rhino offers only the opposite size mode; invoking the active mode would
# start its unrelated measurement command. Account for shared mode memory.
_diameter = False
_CASE_ORDER = ('subd_truncated_cone_cancel_center', 'subd_truncated_cone_default', 'subd_truncated_cone_quad', 'subd_truncated_cone_tri', 'subd_truncated_cone_open', 'subd_truncated_cone_three', 'subd_truncated_cone_four_quad', 'subd_truncated_cone_four_tri', 'subd_truncated_cone_odd', 'subd_truncated_cone_larger', 'subd_truncated_cone_equal', 'subd_truncated_cone_zero', 'subd_truncated_cone_zero_open', 'subd_truncated_cone_negative', 'subd_truncated_cone_free', 'subd_truncated_cone_free_odd', 'subd_truncated_cone_height_point', 'subd_truncated_cone_radius_point', 'subd_truncated_cone_diameter_base', 'subd_truncated_cone_diameter_end', 'subd_truncated_cone_diameter_point', 'subd_truncated_cone_end_point', 'subd_truncated_cone_end_spatial', 'subd_truncated_cone_circumference', 'subd_truncated_cone_area', 'subd_truncated_cone_two_point', 'subd_truncated_cone_three_point', 'subd_truncated_cone_spatial_circle', 'subd_truncated_cone_tilted', 'subd_truncated_cone_three_bands', 'subd_truncated_cone_cancel_radius', 'subd_truncated_cone_cancel_height', 'subd_truncated_cone_cancel_end')
for _key in _CASE_ORDER:
 _case=SCENES[_key]
 _actions=[]
 for _action in _case['actions']:
  if _action in ('Radius','Diameter'):
   _next = _action == 'Diameter'
   if _next == _diameter:continue
   _diameter = _next
  _actions.append(_action)
 _case['actions']=_actions

def request(discovery=False):
 scenes=DISCOVERY if discovery else SCENES
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_truncated_cone_command',id=k,case=k)for k in scenes])
def validate(op):
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_truncated_cone_command'and op['case']in dict(list(DISCOVERY.items())+list(SCENES.items()))and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDTruncatedCone recipe required')
def run(op,host,iterations,tolerance):
 validate(op)
 from subd_box_probe import run_scene
 return run_scene(op,host,iterations,tolerance,'SubDTruncatedCone',dict(list(DISCOVERY.items())+list(SCENES.items()))[op['case']])
