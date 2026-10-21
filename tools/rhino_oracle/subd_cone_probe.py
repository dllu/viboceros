"""Fixed public subdivision-cone constructions in an owned idle document."""
import math

def scene(base,height,around=8,vertical=2,cap='Quad',solid=True,direction='Vertical',**options):
 base=list(base)
 if isinstance(base[0],list)and type(base[1])in(int,float):base[1:2]=['Circumference',2.*math.pi*base[1]]
 return dict(actions=['Solid=Yes','AroundFaces=8','CapFaceStyle='+cap,'AroundFaces='+str(around),'VerticalFaces='+str(vertical),'Solid='+('Yes'if solid else'No')]+base+['DirectionConstraint='+direction,height],**options)

DISCOVERY={
 'cone_discovery_default':dict(actions=[[0.,0.,0.],3.,5.]),
 'cone_discovery_quad':scene([[0.,0.,0.],3.],5.),
 'cone_discovery_tri':scene([[0.,0.,0.],3.],5.,cap='Tri'),
 'cone_discovery_open':scene([[0.,0.,0.],3.],5.,solid=False),
 'cone_discovery_three':scene([[0.,0.,0.],3.],5.,around=3,cap='Tri'),
 'cone_discovery_four':scene([[0.,0.,0.],3.],5.,around=4),
 'cone_discovery_odd':scene([[0.,0.,0.],3.],5.,around=7),
 'cone_discovery_one':scene([[0.,0.,0.],3.],5.,vertical=1),
 'cone_discovery_three_bands':scene([[0.,0.,0.],3.],5.,vertical=3),
 'cone_discovery_negative':scene([[1.,2.,3.],2.],-5.),
 'cone_discovery_free':scene([[0.,0.,0.],3.],[3.,4.,6.],direction='None'),
 'cone_discovery_both':scene([[0.,0.,0.],3.],5.),
 'cone_discovery_orientation':scene([[0.,0.,0.],'Orientation',[1.,1.,1.],3.],5.),
 'cone_discovery_vertical':scene(['Vertical',[0.,0.,0.],3.,[0.,3.,0.]],5.),
}
DISCOVERY['cone_discovery_both']['actions'].insert(-1,'BothSides=Yes')
OPTIONS={}
for _mode,_angles in [('OrBaseAngle',[30.,45.,60.,-30.,0.,90.]),('OrApexAngle',[30.,90.,120.,-90.,0.,180.])]:
 for _angle in _angles:
  _key='cone_option_'+_mode.lower()+'_'+str(int(_angle)).replace('-','minus')
  OPTIONS[_key]=scene([[0.,0.,0.],3.],_mode+'='+str(_angle))
OPTIONS['cone_option_base_separate']=scene([[0.,0.,0.],3.],45.)
OPTIONS['cone_option_base_separate']['actions'].insert(-1,'OrBaseAngle')
OPTIONS['cone_option_apex_separate']=scene([[0.,0.,0.],3.],90.)
OPTIONS['cone_option_apex_separate']['actions'].insert(-1,'OrApexAngle')
OPTIONS['cone_option_around']=dict(curve=[[0.,0.,0.],[0.,0.,10.]],actions=['AroundCurve',{'curve':True},[0.,0.,2.],3.,5.])
SCENES={
 'subd_cone_cancel_center':dict(actions=['Cancel'],cancel=True),
 'subd_cone_default':dict(actions=[[0.,0.,0.],'Enter','Enter']),
 'subd_cone_quad':scene([[0.,0.,0.],3.],5.),
 'subd_cone_tri':scene([[0.,0.,0.],3.],5.,cap='Tri'),
 'subd_cone_open':scene([[1.,2.,3.],2.],5.,solid=False),
 'subd_cone_three':scene([[0.,0.,0.],3.],5.,around=3,cap='Tri'),
 'subd_cone_four_quad':scene([[0.,0.,0.],3.],5.,around=4),
 'subd_cone_four_tri':scene([[0.,0.,0.],3.],5.,around=4,cap='Tri'),
 'subd_cone_six_quad':scene([[0.,0.,0.],3.],5.,around=6),
 'subd_cone_odd':scene([[0.,0.,0.],3.],5.,around=7),
 'subd_cone_three_bands':scene([[0.,0.,0.],3.],5.,vertical=3),
 'subd_cone_negative':scene([[1.,2.,3.],2.],-5.),
 'subd_cone_height_point':scene([[0.,0.,0.],3.],[3.,4.,6.]),
 'subd_cone_free_axis':scene([[0.,0.,0.],3.],[3.,4.,6.],direction='None'),
 'subd_cone_free_odd':scene([[1.,2.,3.],2.],[-2.,-2.,-3.],direction='None',solid=False,around=7),
 'subd_cone_radius_point':scene([[1.,2.,3.],[4.,6.,3.]],5.),
 'subd_cone_radius_spatial_point':scene([[1.,2.,3.],[4.,6.,15.]],5.),
 'subd_cone_diameter':scene([[0.,0.,0.],'Diameter',6.],5.),
 'subd_cone_circumference':scene([[0.,0.,0.],'Circumference',6.*math.pi],5.),
 'subd_cone_area':scene([[0.,0.,0.],'Area',9.*math.pi],5.),
 'subd_cone_two_point':scene(['2Point',[-3.,0.,0.],[3.,0.,0.]],5.),
 'subd_cone_three_point':scene(['3Point',[3.,0.,0.],[0.,3.,0.],[-3.,0.,0.]],5.),
 'subd_cone_spatial_circle':scene(['3Point',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]],4.),
 'subd_cone_tilted':scene([[10.,20.,30.],3.],5.,construction_plane=[[10.,20.,30.],[.6,.8,0.],[0.,0.,1.]]),
 'subd_cone_base_angle':scene([[0.,0.,0.],3.],'OrBaseAngle=30'),
 'subd_cone_base_negative':scene([[1.,2.,3.],3.],'OrBaseAngle=-30'),
 'subd_cone_apex_angle':scene([[0.,0.,0.],3.],'OrApexAngle=90'),
 'subd_cone_apex_negative':scene([[1.,2.,3.],3.],'OrApexAngle=-90'),
 'subd_cone_base_separate':scene([[0.,0.,0.],3.],45.),
 'subd_cone_apex_separate':scene([[0.,0.,0.],3.],120.),
 'subd_cone_cancel_radius':dict(actions=[[0.,0.,0.],'Cancel'],cancel=True),
 'subd_cone_cancel_height':dict(actions=[[0.,0.,0.],3.,'Cancel'],cancel=True),
}
SCENES['subd_cone_base_separate']['actions'].insert(-1,'OrBaseAngle')
SCENES['subd_cone_apex_separate']['actions'].insert(-1,'OrApexAngle')



def request(discovery=False,options=False):
 scenes=OPTIONS if options else DISCOVERY if discovery else SCENES
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_cone_command',id=k,case=k)for k in scenes])
def validate(op):
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_cone_command'and op['case']in dict(list(DISCOVERY.items())+list(OPTIONS.items())+list(SCENES.items()))and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDCone recipe required')
def run(op,host,iterations,tolerance):
 validate(op)
 from subd_box_probe import run_scene
 return run_scene(op,host,iterations,tolerance,'SubDCone',dict(list(DISCOVERY.items())+list(OPTIONS.items())+list(SCENES.items()))[op['case']])
