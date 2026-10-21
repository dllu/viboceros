"""Owned public SubDCylinder modes, cap topologies and axis constraints."""
import math

def scene(base,height,around=8,vertical=2,cap='Quad',solid=True,direction='Vertical',both=False,**options):
 base=list(base)
 if isinstance(base[0],list)and type(base[1])in(int,float):base[1:2]=['Circumference',2.*math.pi*base[1]]
 return dict(actions=['Solid=Yes','AroundFaces=8','CapFaceStyle='+cap,'AroundFaces='+str(around),'VerticalFaces='+str(vertical),'Solid='+('Yes'if solid else'No')]+base+['DirectionConstraint='+direction,'BothSides='+('Yes'if both else'No'),height],**options)

SCENES={
 'subd_cylinder_cancel_center':dict(actions=['Cancel'],cancel=True),
 'subd_cylinder_cancel_height':dict(actions=[[0.,0.,0.],3.,'Cancel'],cancel=True),
 'subd_cylinder_default':dict(actions=[[0.,0.,0.],3.,5.]),
 'subd_cylinder_quad':scene([[0.,0.,0.],3.],5.),
 'subd_cylinder_open':scene([[1.,2.,3.],2.],5.,solid=False),
 'subd_cylinder_tri':scene([[0.,0.,0.],3.],5.,cap='Tri'),
 'subd_cylinder_three':scene([[0.,0.,0.],3.],5.,around=3,cap='Tri'),
 'subd_cylinder_four':scene([[0.,0.,0.],3.],5.,around=4),
 'subd_cylinder_odd':scene([[0.,0.,0.],3.],5.,around=7),
 'subd_cylinder_one_band':scene([[0.,0.,0.],3.],5.,vertical=1),
 'subd_cylinder_negative':scene([[1.,2.,3.],2.],-5.),
 'subd_cylinder_both':scene([[1.,2.,3.],2.],5.,both=True),
 'subd_cylinder_height_point':scene([[0.,0.,0.],3.],[3.,4.,6.]),
 'subd_cylinder_free_axis':scene([[0.,0.,0.],3.],[3.,4.,6.],direction='None'),
 'subd_cylinder_free_negative':scene([[1.,2.,3.],2.],[-2.,-2.,-3.],direction='None',solid=False),
 'subd_cylinder_radius_point':scene([[1.,2.,3.],[4.,6.,3.]],5.),
 'subd_cylinder_diameter':scene([[0.,0.,0.],'Diameter',6.],5.),
 'subd_cylinder_circumference':scene([[0.,0.,0.],'Circumference',18.84955592153876],5.),
 'subd_cylinder_area':scene([[0.,0.,0.],'Area',28.274333882308138],5.),
 'subd_cylinder_two_point':scene(['2Point',[-3.,0.,0.],[3.,0.,0.]],5.),
 'subd_cylinder_three_point':scene(['3Point',[3.,0.,0.],[0.,3.,0.],[-3.,0.,0.]],5.),
 'subd_cylinder_spatial_circle':scene(['3Point',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]],4.),
 'subd_cylinder_vertical_circle':scene(['Vertical',[0.,0.,0.],3.,[0.,3.,0.]],5.),
 'subd_cylinder_orientation':scene([[0.,0.,0.],'Orientation',[1.,1.,1.],3.],5.),
 'subd_cylinder_tilted':scene([[10.,20.,30.],3.],5.,construction_plane=[[10.,20.,30.],[.6,.8,0.],[0.,0.,1.]]),
}
SCENES['subd_cylinder_quad']['actions']=['AroundFaces=8','CapFaceStyle=Quad','VerticalFaces=2','Solid=Yes',[0.,0.,0.],3.,'DirectionConstraint=Vertical','BothSides=No',5.]
SCENES['subd_cylinder_free_odd']=scene([[1.,2.,3.],2.],[-2.,-2.,-3.],direction='None',solid=False,around=7)
SCENES['subd_cylinder_free_x']=scene([[0.,0.,0.],3.],[5.,0.,0.],direction='None',solid=False,around=7)

SCENES['subd_cylinder_free_z']=scene([[0.,0.,0.],3.],[0.,0.,5.],direction='None',solid=False,around=7)

for _name in ['one_band','vertical_circle','orientation']:
 SCENES['subd_cylinder_'+_name]['unsupported']=True

def request():
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_cylinder_command',id=name,case=name)for name in SCENES])
def validate(op):
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_cylinder_command'and op['case']in SCENES and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDCylinder recipe required')
def run(op,host,iterations,tolerance):
 validate(op)
 from subd_box_probe import run_scene
 return run_scene(op,host,iterations,tolerance,'SubDCylinder',SCENES[op['case']])
