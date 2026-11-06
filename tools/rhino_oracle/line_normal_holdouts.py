"""Independent numeric, zero-curvature and trimmed surface normal inputs."""
if __package__:from .line_normal_cases import case,CIRCLE,PLANE,CURVED,QUAD
else:from line_normal_cases import case,CIRCLE,PLANE,CURVED,QUAD
LINE=dict(type='line',start=[0,0,0],end=[10,0,0])
POLYLINE=dict(type='polyline',vertices=[[0,0,0],[5,0,0],[5,5,0]])
def patch(kind,flip=False):
 return dict(type='normal_surface',target=dict(kind=kind,points=[[0,0,0],[10,0,0],[10,10,0],[0,10,0]],aim=[2,3,0],view='Top',flip=flip))
def trimmed(ignore,flip=False):
 r=case(patch('trimmed',flip),[2,3,0],[8,3,2],[8,3,7]);r['actions'].insert(1,'IgnoreTrims='+('Yes'if ignore else'No'));return r
HOLDOUTS={
 'line_point':case(LINE,[5,0,0],[5,0,0],[7,4,3]),
 'polyline_point':case(POLYLINE,[3,0,0],[3,0,0],[7,4,3]),
 'polyline_vertex':case(POLYLINE,[3,0,0],[5,0,0],[7,4,3]),
 'line_length':case(LINE,[5,0,0],[5,0,0],5),
 'circle_numeric':case(CIRCLE,[4.99,.2,0],[3,4,0],5),
 'circle_negative':case(CIRCLE,[4.99,.2,0],[3,4,0],-5),
 'circle_numeric_hover':case(CIRCLE,[4.99,.2,0],[3,4,0],dict(hover=[8,9,0]),5),
 'circle_numeric_inward':case(CIRCLE,[4.99,.2,0],[3,4,0],dict(hover=[0,0,0]),5),
 'plane_numeric':case(PLANE,[5,5,0],[4,3,0],5),
 'plane_numeric_hover':case(PLANE,[5,5,0],[4,3,0],dict(hover=[8,9,0]),5),
 'quadratic_numeric':case(QUAD,[4.5,3,0],[4.5,3,0],5),
 'surface_reversed':case(patch('brep',True),[2,3,0],[4,3,2],[8,9,10]),
 'trimmed_no':trimmed(False),
 'trimmed_yes':trimmed(True),
 'trimmed_flipped':trimmed(False,True),
 'ignore_reset':dict(curves=[PLANE],actions=['Normal','IgnoreTrims=No',dict(curve=0,pick=[5,5,0]),[4,3,2],[8,9,10]]),
 'normal_redo_cancel':dict(case(CIRCLE,[4.99,.2,0],[3,4,0],'Cancel'),pending_redo=True),
 'normal_redo_empty':dict(actions=['Normal','Cancel'],pending_redo=True),
}
