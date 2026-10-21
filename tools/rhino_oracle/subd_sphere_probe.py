"""Owned public SubDSphere style and sphere-construction recipes."""
DISCOVERY = {
 'subd_sphere_discovery_default':dict(actions=[[0.,0.,0.],3.]),
 'subd_sphere_discovery_styles_quad':dict(actions=['Styles','Quads','Subdivisions=0',[0.,0.,0.],3.]),
 'subd_sphere_discovery_quad':dict(actions=['Quads','Subdivisions=1',[0.,0.,0.],3.]),
 'subd_sphere_discovery_tri':dict(actions=['Triangles','Subdivisions=0',[0.,0.,0.],3.]),
 'subd_sphere_discovery_uv':dict(actions=['UV','AroundFaces=8','VerticalFaces=4',[0.,0.,0.],3.]),
 'subd_sphere_discovery_style_quad':dict(actions=['Style=Quads','Subdivisions=0',[0.,0.,0.],3.]),
 'subd_sphere_discovery_style_tri':dict(actions=['Styles','Triangles','Subdivisions=1',[0.,0.,0.],3.]),
 'subd_sphere_discovery_style_uv':dict(actions=['Styles','UV','AroundFaces=7','VerticalFaces=3',[0.,0.,0.],3.]),
}
STYLES = {
 'subd_sphere_styles_quad_zero':dict(actions=['Style=Quad','Subdivisions=0',[0.,0.,0.],3.]),
 'subd_sphere_styles_quad_two':dict(actions=['Style=Quad','Subdivisions=2',[0.,0.,0.],3.]),
 'subd_sphere_styles_uv':dict(actions=['Style=Quad','Style=UV','AroundFaces=8','VerticalFaces=4',[0.,0.,0.],3.]),
 'subd_sphere_styles_tri':dict(actions=['Style=Quad','Style=Tri','Subdivisions=0',[0.,0.,0.],3.]),
 'subd_sphere_styles_triangle':dict(actions=['Style=Quad','Style=Triangle','Subdivisions=1',[0.,0.,0.],3.]),
 'subd_sphere_styles_globe':dict(actions=['Style=Quad','Style=Globe','AroundFaces=7','VerticalFaces=3',[0.,0.,0.],3.]),
}
def sphere(actions,style='Quad',subdivisions=1,around=8,vertical=4,**options):
 prefix=['Style='+style]+(['AroundFaces='+str(around),'VerticalFaces='+str(vertical)]if style=='UV'else['Subdivisions='+str(subdivisions)])
 # A one-shot circumference avoids changing the native Radius/Diameter mode.
 if len(actions)==2 and isinstance(actions[0],list)and type(actions[1])in(int,float):
  import math
  actions=[actions[0],'Circumference',2.*math.pi*actions[1]]
 return dict(actions=prefix+actions,**options)
SCENES = {
 'subd_sphere_cancel_center':dict(actions=['Cancel'],cancel=True),
 'subd_sphere_default':dict(actions=[[0.,0.,0.],3.]),
 'subd_sphere_quad_zero':sphere([[0.,0.,0.],3.],subdivisions=0),
 'subd_sphere_quad_one':sphere([[0.,0.,0.],3.]),
 'subd_sphere_quad_two':sphere([[0.,0.,0.],3.],subdivisions=2),
 'subd_sphere_quad_three':sphere([[0.,0.,0.],3.],subdivisions=3),
 'subd_sphere_uv':sphere([[0.,0.,0.],3.],style='UV'),
 'subd_sphere_uv_three':sphere([[0.,0.,0.],3.],style='UV',around=3,vertical=2),
 'subd_sphere_uv_four':sphere([[0.,0.,0.],3.],style='UV',around=4,vertical=2),
 'subd_sphere_uv_odd':sphere([[0.,0.,0.],3.],style='UV',around=7,vertical=3),
 'subd_sphere_tri_zero':sphere([[0.,0.,0.],3.],style='Triangle',subdivisions=0),
 'subd_sphere_tri_one':sphere([[0.,0.,0.],3.],style='Triangle'),
 'subd_sphere_tri_two':sphere([[0.,0.,0.],3.],style='Triangle',subdivisions=2),
 'subd_sphere_radius_point':sphere([[1.,2.,3.],[4.,6.,15.]]),
 'subd_sphere_diameter':sphere([[0.,0.,0.],'Diameter',6.]),
 'subd_sphere_circumference':sphere([[1.,2.,3.],'Circumference',18.84955592153876]),
 'subd_sphere_area':sphere([[1.,2.,3.],'Area',28.274333882308138]),
 'subd_sphere_two_point':sphere(['2Point',[-4.,0.,0.],[4.,0.,0.]]),
 'subd_sphere_two_point_spatial':sphere(['2Point',[1.,2.,3.],[5.,8.,9.]]),
 'subd_sphere_three_point':sphere(['3Point',[3.,0.,0.],[0.,3.,0.],[-3.,0.,0.]]),
 'subd_sphere_spatial_circle':sphere(['3Point',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]]),
 'subd_sphere_orientation':sphere([[1.,2.,3.],'Orientation',[2.,3.,6.],'Circumference',18.84955592153876]),
 'subd_sphere_four_point':sphere(['4Point',[3.,0.,0.],[0.,3.,0.],[-3.,0.,0.],[0.,0.,3.]]),
 'subd_sphere_four_point_offset':sphere(['4Point',[3.,2.,3.],[1.,4.,3.],[-1.,2.,3.],[1.,2.,9.]]),
 'subd_sphere_four_point_radius':sphere(['4Point',[3.,0.,0.],[0.,3.,0.],[-3.,0.,0.],'Radius',4.,[0.,0.,10.]]),
 'subd_sphere_three_point_radius':sphere(['3Point',[-2.,0.,0.],[2.,0.,0.],'Radius',3.,[0.,5.,0.]]),
 'subd_sphere_tilted':sphere([[10.,20.,30.],3.],construction_plane=[[10.,20.,30.],[.6,.8,0.],[0.,0.,1.]]),
 'subd_sphere_around_z':sphere(['AroundCurve',{'curve':True},[0.,0.,2.],'Circumference',18.84955592153876],curve=[[0.,0.,0.],[0.,0.,10.]]),
 'subd_sphere_cancel_radius':sphere([[1.,2.,3.],'Cancel'],cancel=True),
}


def request(discovery=False,styles=False):
 cases=STYLES if styles else DISCOVERY if discovery else SCENES
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_sphere_command',id=k,case=k)for k in cases])

def validate(op):
 cases=dict(DISCOVERY);cases.update(STYLES);cases.update(SCENES)
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_sphere_command'and op['case']in cases and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDSphere recipe required')

def run(op,host,iterations,tolerance):
 validate(op)
 from subd_box_probe import run_scene
 cases=dict(DISCOVERY);cases.update(STYLES);cases.update(SCENES)
 return run_scene(op,host,iterations,tolerance,'SubDSphere',cases[op['case']])
