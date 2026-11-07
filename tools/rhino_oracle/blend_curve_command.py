"""Fixed native adjustable-curve-blend command recipes."""
A=dict(type='line',start=[-5.,0.,0.],end=[0.,0.,0.])
B=dict(type='line',start=[10.,3.,0.],end=[15.,3.,0.])
def pick(i,p):return dict(curve=i,pick=p)
def case(actions,curves=None,**kwargs):return dict(curves=curves or[A,B],view='Top',actions=actions,**kwargs)
PILOT={'adjustable_default':case([pick(0,[-.25,0.,0.]),pick(1,[10.25,3.,0.]),'Enter'])}
CASES=dict(PILOT)
def validate(op):
 if not isinstance(op,dict)or set(op)!={'op','id','case'}or op.get('op')!='blend_curve_command'or not isinstance(op.get('case'),str)or op['case']not in CASES or op.get('id')!=op['case']:raise ValueError('invalid fixed BlendCrv command recipe')
def request(names=None):return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=[dict(op='blend_curve_command',id=k,case=k)for k in(PILOT if names is None else names)])

def adjustable(name,first=2,second=2,curves=None,picks=None,extra=None):
 sources=curves or[A,B];points=picks or[[-.25,0.,0.],[10.25,3.,0.]]
 return case(['Continuity_1=G'+str(first),'Continuity_2=G'+str(second)]+[pick(i,p)for i,p in enumerate(points)]+['Trim=No','Join=No']+(extra or[])+['Enter'],curves=sources)
def nurbs(points,weights=None):
 return dict(type='nurbs',degree=3,control_points=[dict(point=p,weight=w)for p,w in zip(points,weights or[1.]*len(points))],knots=[0.,0.,0.,0.,1.,1.,1.,1.])
ARC_A=dict(type='arc',points=[[-5.,-5.,0.],[-3.,-1.,0.],[0.,0.,0.]])
CUBIC_A=nurbs([[-6.,-3.,0.],[-4.,-3.,0.],[-3.,0.,0.],[0.,0.,0.]])
CUBIC_B=nurbs([[10.,3.,0.],[13.,3.,0.],[14.,6.,0.],[16.,7.,0.]])
DISCOVERY={
 'g2_lines':adjustable('g2_lines'),
 'g1_lines':adjustable('g1_lines',1,1),
 'g0_lines':adjustable('g0_lines',0,0),
 'g0_g1_lines':adjustable('g0_g1_lines',0,1),
 'g1_g2_lines':adjustable('g1_g2_lines',1,2),
 'g2_g0_lines':adjustable('g2_g0_lines',2,0),
 'g3_lines':adjustable('g3_lines',3,3),
 'g4_lines':adjustable('g4_lines',4,4),
 'reverse_ends':adjustable('reverse_ends',picks=[[-4.75,0.,0.],[14.75,3.,0.]]),
 'arc_line':adjustable('arc_line',curves=[ARC_A,B],picks=[[-.25,-.006,0.],[10.25,3.,0.]]),
 'cubic_curves':adjustable('cubic_curves',curves=[CUBIC_A,CUBIC_B]),
 'trim_defaults':adjustable('trim_defaults',extra=['Trim=Yes','Join=No']),
 'join_defaults':adjustable('join_defaults',extra=['Trim=Yes','Join=Yes']),
 'cancel_first':case(['Cancel'],pending_redo=True),
 'cancel_second':case([pick(0,[-.25,0.,0.]),'Cancel'],pending_redo=True),
}
CASES.update(DISCOVERY)

def held(first=2,second=2,curves=None,picks=None,start='CurveEnd',extra=None):
 value=adjustable('',first,second,curves,picks,extra)
 value['actions'].insert(0,'BlendStart='+start)
 return value
HOLDOUTS={
 'g3_arc':held(3,3,curves=[ARC_A,B],picks=[[-.25,-.006,0.],[10.25,3.,0.]]),
 'g4_arc':held(4,4,curves=[ARC_A,B],picks=[[-.25,-.006,0.],[10.25,3.,0.]]),
 'g3_cubic':held(3,3,curves=[CUBIC_A,CUBIC_B]),
 'g4_cubic':held(4,4,curves=[CUBIC_A,CUBIC_B]),
 'pick_point':held(start='PickPoint',picks=[[-2.,0.,0.],[12.,3.,0.]]),
 'pick_point_trim':held(start='PickPoint',picks=[[-2.,0.,0.],[12.,3.,0.]],extra=['Trim=Yes']),
 'pick_point_join':held(start='PickPoint',picks=[[-2.,0.,0.],[12.,3.,0.]],extra=['Trim=Yes','Join=Yes']),
 'join_without_trim':held(extra=['Join=Yes']),
 'g2_handle_start':held(extra=[dict(click=[1.8,0.,0.]),[3.,0.,0.]]),
 'g1_handle_start':held(1,1,extra=[dict(click=[3.1,0.,0.]),[4.,0.,0.]]),
 'flip_by_pick':held(extra=['Flip',pick(0,[-.25,0.,0.])]),
 'flip_by_number':held(extra=['Flip',1.]),
}
CASES.update(HOLDOUTS)

ADJUSTMENTS={
 'shape_start_g2':held(extra=[dict(click=[1.8,0.,0.]),[3.,0.,0.]]),
 'shape_start_g1':held(1,1,extra=[dict(click=[3.1,0.,0.]),[4.,0.,0.]]),
 'shape_end_g2':held(extra=[dict(click=[8.2,3.,0.]),[7.,3.,0.]]),
 'slide_start':held(extra=[dict(click=[0.,0.,0.]),[-1.,0.,0.]]),
 'flip_both':held(extra=['Flip','Enter']),
 'pick_point_near_start':held(start='PickPoint',picks=[[-4.,0.,0.],[14.,3.,0.]]),
}
CASES.update(ADJUSTMENTS)
