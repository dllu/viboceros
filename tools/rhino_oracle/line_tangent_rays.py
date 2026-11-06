"""Prescribed physical mouse picks on elevated, tilted and spatial guides."""
CIRCLE=dict(type='circle',center=[0.,0.,3.],radius=5.,x_axis=[1.,0.,0.],normal=[0.,0.,1.])
TILTED=dict(type='circle',center=[2.,-3.,4.],radius=5.,x_axis=[1.,0.,0.],normal=[0.,-.6,.8])
CUBIC=dict(type='nurbs',degree=3,knots=[0.,0.,0.,0.,1.,1.,1.,1.],control_points=[dict(point=p,weight=1.)for p in [[0.,0.,0.],[2.,5.,3.],[7.,-2.,5.],[10.,4.,1.]]])
ARC=dict(type='arc',points=[[5.,0.,3.],[3.5355339059327378,3.5355339059327378,3.],[0.,5.,3.]])
def case(source,pick,aim,view='Top',before=None,both=False,planar=False):
 return dict(curves=[source],view=view,planar=planar,actions=(['BothSides']if both else[])+['Tangent',dict(curve=0,pick=pick)]+([]if before is None else[before])+[dict(click=aim)])
CASES={
 'ray_elevated_top':case(CIRCLE,[4.99,.2,3.],[8.,6.,3.]),
 'ray_elevated_far':case(CIRCLE,[-4.99,.2,3.],[8.,6.,3.]),
 'ray_vertical_front':case(dict(CIRCLE,center=[1.,2.,3.],normal=[0.,1.,0.]),[5.99,2.,2.8],[9.,2.,9.],view='Front'),
 'ray_tilted_perspective':case(TILTED,[6.99,-2.8,4.15],[10.,1.8,7.6],view='Perspective'),
 'ray_cubic_top':case(CUBIC,[3.52,1.84,2.8],[6.22,1.39,3.85]),
 'ray_cubic_perspective':case(CUBIC,[3.52,1.84,2.8],[6.22,1.39,3.85],view='Perspective'),
 'ray_fixed_elevated':case(CIRCLE,[4.99,.2,3.],[8.,6.,3.],before='FromFirstPoint',view='Perspective'),
 'ray_signed_length':case(CIRCLE,[4.99,.2,3.],[8.,6.,3.],before=-5.),
 'ray_bothsides':case(CIRCLE,[4.99,.2,3.],[8.,6.,3.],both=True),
 'ray_planar_elevated':case(CIRCLE,[4.99,.2,3.],[8.,6.,3.],planar=True),
 'ray_arc_boundary':case(ARC,[3.5355339059327378,3.5355339059327378,3.],[-8.,2.,3.]),
 'ray_line_boundary':case(dict(type='line',start=[0.,0.,3.],end=[10.,0.,3.]),[5.,0.,3.],[15.,4.,3.]),
}
REJECTED=[k for k in CASES if k not in ['ray_fixed_elevated','ray_planar_elevated','ray_arc_boundary','ray_line_boundary']]
CONFIRM={k.replace('ray_','mouse_'):dict(CASES[k],actions=CASES[k]['actions']+['Cancel'],pending_redo=True)for k in REJECTED}
def confirmation_request():return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='line_tangent_mouse',id=k,case=k)for k in CONFIRM])
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')not in ['line_tangent_rays','line_tangent_mouse']or not isinstance(op.get('case'),str)or op.get('case')not in (CONFIRM if op['op']=='line_tangent_mouse'else CASES) or op.get('id')!=op['case']:raise ValueError('invalid fixed tangent viewing-ray recipe')
def request():return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='line_tangent_rays',id=k,case=k)for k in CASES])
