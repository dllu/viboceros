"""Fixed public subdivision-torus construction recipes in an owned document."""
import math

def scene(base, tube, around=8, vertical=6, **kw):
 base=list(base)
 if isinstance(base[0],list)and type(base[1])in(int,float):base[1:2]=['Circumference',2.*math.pi*base[1]]
 return dict(actions=['AroundFaces='+str(around),'VerticalFaces='+str(vertical)]+base+(tube if isinstance(tube,list)and tube and isinstance(tube[0],str)else[tube]),**kw)
DISCOVERY={
 'torus_discovery_default':dict(actions=[[0.,0.,0.],3.,1.]),
 'torus_discovery_grid':scene([[0.,0.,0.],3.],1.),
 'torus_discovery_three':scene([[0.,0.,0.],3.],1.,around=3,vertical=3),
 'torus_discovery_odd':scene([[1.,2.,3.],5.],2.,around=7,vertical=5),
 'torus_discovery_point':scene([[0.,0.,0.],3.],[4.,0.,2.]),
 'torus_discovery_inner':scene([[0.,0.,0.],3.],['FixInnerDimension',1.]),
 'torus_discovery_diameter':scene([[0.,0.,0.],3.],['Diameter',2.]),
 'torus_discovery_radius':scene([[0.,0.,0.],3.],['Radius',1.]),
 'torus_discovery_vertical_two':scene([[0.,0.,0.],3.],1.,vertical=2),
 'torus_discovery_horn':scene([[0.,0.,0.],3.],3.),
 'torus_discovery_spindle':scene([[0.,0.,0.],3.],4.),
 'torus_discovery_zero':scene([[0.,0.,0.],3.],0.),
 'torus_discovery_free_circle':scene(['3Point',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]],1.),
}
def sized(base,tube,inner=False,**kw):
 case=scene(base,tube,**kw)
 index=len(case['actions'])-len(tube)if isinstance(tube,list)and tube and isinstance(tube[0],str)else len(case['actions'])-1
 case['actions'].insert(index,'FixInnerDimension='+('Yes'if inner else'No'))
 return case
SIZES={
 'torus_sizes_horn':sized([[0.,0.,0.],3.],3.),
 'torus_sizes_spindle':sized([[0.,0.,0.],3.],4.),
 'torus_sizes_zero':sized([[0.,0.,0.],3.],0.),
 'torus_sizes_zero_offset':sized([[1.,2.,3.],3.],0.),
 'torus_sizes_negative':sized([[0.,0.,0.],3.],-1.),
 'torus_sizes_inner_point':sized([[0.,0.,0.],3.],[2.,0.,9.]),
 'torus_sizes_center_point':sized([[0.,0.,0.],3.],[0.,0.,9.]),
 'torus_sizes_quadrant_point':sized([[0.,0.,0.],3.],[0.,4.,9.]),
 'torus_sizes_fix_point':sized([[0.,0.,0.],3.],[7.,0.,9.],inner=True),
 'torus_sizes_fix_below':sized([[0.,0.,0.],3.],[1.,0.,9.],inner=True),
 'torus_sizes_fix_zero':sized([[0.,0.,0.],3.],0.,inner=True),
 'torus_sizes_diameter_point':sized([[0.,0.,0.],3.],['Diameter',[5.,0.,9.]]),
 'torus_sizes_radius_restore':sized([[0.,0.,0.],3.],['Radius',1.]),
}
OPTIONS={
 'torus_options_default':dict(actions=[[0.,0.,0.],'Enter','Enter']),
 'torus_options_exact_horn':sized([[0.,0.,0.],'Radius',3.],3.),
 'torus_options_exact_spindle':sized([[0.,0.,0.],3.],4.),
 'torus_options_diameter_large':sized([[0.,0.,0.],3.],['Diameter',4.]),
 'torus_options_diameter_equal':sized([[0.,0.,0.],3.],6.),
 'torus_options_diameter_default':sized([[0.,0.,0.],3.],'Enter'),
 'torus_options_radius_default':sized([[0.,0.,0.],3.],['Radius','Enter']),
 'torus_options_fix_default':sized([[0.,0.,0.],3.],'Enter',inner=True),
 'torus_options_base_diameter':sized([[0.,0.,0.],'Diameter',6.],1.),
 'torus_options_base_default':dict(actions=['AroundFaces=8','VerticalFaces=6',[0.,0.,0.],'Enter','FixInnerDimension=No','Enter']),
 'torus_options_fix_base':dict(actions=['FixInnerDimension=Yes',[0.,0.,0.],3.,1.]),
 'torus_options_late_count':dict(actions=[[0.,0.,0.],3.,'AroundFaces=12',1.]),
}
SCENES={
 'subd_torus_cancel_center':dict(actions=['Cancel'],cancel=True),
 'subd_torus_default_small_retry':dict(actions=[[0.,0.,0.],'Enter','Enter',.5]),
 'subd_torus_default':dict(actions=[[0.,0.,0.],3.,1.]),
 'subd_torus_grid':sized([[0.,0.,0.],3.],1.),
 'subd_torus_three':sized([[0.,0.,0.],3.],1.,around=3,vertical=3),
 'subd_torus_four':sized([[0.,0.,0.],3.],1.,around=4,vertical=4),
 'subd_torus_odd':sized([[1.,2.,3.],5.],2.,around=7,vertical=5),
 'subd_torus_tall_grid':sized([[0.,0.,0.],3.],1.,around=3,vertical=8),
 'subd_torus_flat_grid':sized([[0.,0.,0.],3.],1.,around=8,vertical=3),
 'subd_torus_dense':sized([[0.,0.,0.],5.],2.,around=12,vertical=9),
 'subd_torus_tube_point':sized([[0.,0.,0.],3.],[4.,0.,0.]),
 'subd_torus_tube_spatial':sized([[0.,0.,0.],3.],[4.,0.,9.]),
 'subd_torus_tube_inner':sized([[0.,0.,0.],3.],[2.,0.,9.]),
 'subd_torus_tube_quadrant':sized([[0.,0.,0.],3.],[0.,4.,9.]),
 'subd_torus_fix_inner':sized([[0.,0.,0.],3.],1.,inner=True),
 'subd_torus_fix_large':sized([[0.,0.,0.],3.],4.,inner=True),
 'subd_torus_fix_point':sized([[0.,0.,0.],3.],[7.,0.,9.],inner=True),
 'subd_torus_fix_below':sized([[0.,0.,0.],3.],[1.,0.,9.],inner=True),
 'subd_torus_fix_toggle':sized([[0.,0.,0.],3.],['FixInnerDimension',1.]),
 'subd_torus_near_horn':sized([[0.,0.,0.],3.],3.),
 'subd_torus_zero_offset':sized([[1.,2.,3.],3.],0.),
 'subd_torus_radius_point':sized([[1.,2.,3.],[4.,6.,15.]],1.),
 'subd_torus_orientation':sized([[1.,2.,3.],'Orientation',[2.,3.,6.],'Circumference',6.*math.pi],1.),
 'subd_torus_vertical':sized(['Vertical',[0.,0.,0.],3.,[0.,3.,0.]],1.),
 'subd_torus_diameter_base':sized([[0.,0.,0.],'Diameter',6.],1.),
 'subd_torus_diameter_tube':sized([[0.,0.,0.],3.],['Diameter',4.]),
 'subd_torus_diameter_point':sized([[0.,0.,0.],3.],[5.,0.,9.]),
 'subd_torus_diameter_default':sized([[0.,0.,0.],3.],'Enter'),
 'subd_torus_radius_default':sized([[0.,0.,0.],3.],['Radius','Enter']),
 'subd_torus_base_default':dict(actions=[[0.,0.,0.],'Enter','FixInnerDimension=No',1.]),
 'subd_torus_circumference':sized([[0.,0.,0.],'Circumference',6.*math.pi],1.),
 'subd_torus_area':sized([[0.,0.,0.],'Area',9.*math.pi],1.),
 'subd_torus_two_point':sized(['2Point',[-3.,0.,0.],[3.,0.,0.]],1.),
 'subd_torus_two_spatial':sized(['2Point',[1.,2.,3.],[5.,8.,9.]],1.),
 'subd_torus_three_point':sized(['3Point',[3.,0.,0.],[0.,3.,0.],[-3.,0.,0.]],1.),
 'subd_torus_spatial_circle':sized(['3Point',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]],1.),
 'subd_torus_three_point_radius':sized(['3Point',[-2.,0.,0.],[2.,0.,0.],'Radius',3.,[0.,5.,0.]],1.),
 'subd_torus_tilted':sized([[10.,20.,30.],3.],1.,construction_plane=[[10.,20.,30.],[.6,.8,0.],[0.,0.,1.]]),
 'subd_torus_around_z':sized(['AroundCurve',{'curve':True},[0.,0.,2.],'Circumference',6.*math.pi],1.,curve=[[0.,0.,0.],[0.,0.,10.]]),
 'subd_torus_around_x':sized(['AroundCurve',{'curve':True},[2.,0.,0.],'Circumference',6.*math.pi],1.,around=7,curve=[[0.,0.,0.],[10.,0.,0.]]),
 'subd_torus_cancel_radius':dict(actions=[[0.,0.,0.],'Cancel'],cancel=True),
 'subd_torus_cancel_two_point':dict(actions=['2Point',[-3.,0.,0.],'Cancel'],cancel=True),
 'subd_torus_cancel_tube':sized([[0.,0.,0.],3.],'Cancel',cancel=True),
 'subd_torus_reject_equal':dict(actions=['AroundFaces=8','VerticalFaces=6',[0.,0.,0.],3.,'FixInnerDimension=No',3.,'Cancel'],cancel=True),
 'subd_torus_reject_large':sized([[0.,0.,0.],3.],[ 'FixInnerDimension=No',4.,'Cancel'],cancel=True),
 'subd_torus_reject_negative':sized([[0.,0.,0.],3.],['FixInnerDimension=No',-1.,'Cancel'],cancel=True),
 'subd_torus_reject_zero':sized([[0.,0.,0.],3.],['FixInnerDimension=No',0.,'Cancel'],cancel=True),
 'subd_torus_reject_center':sized([[0.,0.,0.],3.],['FixInnerDimension=No',[0.,0.,9.],'Cancel'],cancel=True),
}
def request(discovery=False,sizes=False,options=False):
 cases=OPTIONS if options else SIZES if sizes else DISCOVERY if discovery else SCENES
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_torus_command',id=k,case=k)for k in cases])
def validate(op):
 cases=dict(DISCOVERY);cases.update(OPTIONS);cases.update(SIZES);cases.update(SCENES)
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_torus_command'and op['case']in cases and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDTorus recipe required')
def run(op,host,iterations,tolerance):
 validate(op)
 from subd_box_probe import run_scene
 cases=dict(DISCOVERY);cases.update(OPTIONS);cases.update(SIZES);cases.update(SCENES)
 return run_scene(op,host,iterations,tolerance,'SubDTorus',cases[op['case']])
