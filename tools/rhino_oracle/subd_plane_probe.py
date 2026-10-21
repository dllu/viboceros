"""Fixed public SubDPlane commands with native prompt and limit-surface evidence."""
from copy import deepcopy

def scene(actions,counts=None,**options):
 result=dict(actions=deepcopy(actions),**options)
 if counts is not None:
  result.update(counts=counts)
 return result
SCENES={
 'subd_plane_default':scene([[0.,0.,0.],[4.,3.,0.]]),
 'subd_plane_one':scene([[0.,0.,0.],[4.,3.,0.]],[1,1]),
 'subd_plane_grid':scene([[1.,2.,3.],[5.,8.,3.]],[2,3]),
 'subd_plane_reversed':scene([[5.,8.,3.],[1.,2.,3.]],[2,3]),
 'subd_plane_off_plane':scene([[0.,0.,2.],[4.,3.,8.]],[2,3]),
 'subd_plane_center':scene(['Center',[1.,2.,3.],[3.,5.,3.]],[2,3]),
 'subd_plane_three_point':scene(['3Point',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]],[2,3]),
 'subd_plane_three_point_late':scene([[1.,2.,3.],'3Point',[4.,6.,3.],[0.,5.,5.]],[2,3]),
 'subd_plane_midpoint':scene(['EdgeMidpoint',[1.,2.,3.],[4.,6.,3.],[0.,5.,5.]],[2,3]),
 'subd_plane_vertical':scene(['Vertical',[1.,2.,3.],[4.,6.,3.],[0.,5.,8.]],[2,3]),
 'subd_plane_dimensions':scene([[0.,0.,0.],4.,3.],[2,3]),
 'subd_plane_zero_origin':scene([0,[4.,3.,0.]],[2,3]),
 'subd_plane_uniform_first':scene([3,[0.,0.,0.],[4.,3.,0.]]),
 'subd_plane_uniform_end':scene([[0.,0.,0.],[4.,3.,0.],3]),
 'subd_plane_prefix_counts':dict(counts=[2,3],actions=[[0.,0.,0.],[4.,3.,0.],'Enter']),
 'subd_plane_tilted':scene([[10.,20.,30.],[12.4,23.2,36.]],[2,3],construction_plane=[[10.,20.,30.],[.6,.8,0.],[0.,0.,1.]]),
 'subd_plane_around_curve':scene(['AroundCurve',{'curve':True},[0.,0.,2.],[2.,3.,2.]],[2,3],curve=[[0.,0.,0.],[0.,0.,10.]]),
 'subd_plane_cancel_corner':dict(actions=[[0.,0.,0.],'Cancel'],cancel=True),
 'subd_plane_cancel_grid':dict(actions=[[0.,0.,0.],[4.,3.,0.],'XCount=2','YCount=3','Cancel'],cancel=True),
}
SCENES['subd_plane_midpoint']['unsupported']=True
SCENES['subd_plane_uniform_end']['unsupported']=True
SCENES['subd_plane_cancel_grid'].pop('cancel')
SCENES['subd_plane_cancel_grid']['unsupported']=True

def request():
 return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=[dict(op='subd_plane_command',id=name,case=name)for name in SCENES])
def validate(op):
 try:valid=isinstance(op,dict)and set(op)=={'op','id','case'}and op['op']=='subd_plane_command'and op['case']in SCENES and op['id']==op['case']
 except TypeError:valid=False
 if not valid:raise ValueError('fixed SubDPlane recipe required')
def run(op,host,iterations,tolerance):
 validate(op)
 from subd_box_probe import run_scene
 return run_scene(op,host,iterations,tolerance,'SubDPlane',SCENES[op['case']])
