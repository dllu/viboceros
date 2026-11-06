"""Fixed public plane-input discovery and qualification recipes."""
from copy import deepcopy

POINTS = [[-4., -3., 5.], [-2., 2., -1.], [0., -1.5, 3.], [2., 3., 7.], [4., -2., 2.]]
CASES = {
    'world_xy': ('_WorldXY', POINTS, '_Enter'),
    'world_yz': ('_WorldYZ', [[5., -4., -3.], [-1., -2., 2.], [3., 0., -1.5], [7., 2., 3.], [2., 4., -2.]], '_Enter'),
    'through': ('_ThroughPoint w0,0,4', POINTS, '_Enter'),
    'three_point': ('_3Point w0,0,2 w10,0,2 w0,10,2', POINTS, '_Enter'),
    'vertical': ('_Vertical w0,0,0 w10,0,0 w0,0,5', POINTS, '_Enter'),
    'default': ('_Enter', POINTS, '_Enter'),
    'close': ('_WorldXY', POINTS[:4], '_Close'),
    'undo': ('_WorldXY', POINTS[:3] + ['_Undo'] + POINTS[3:], '_Enter'),
    'cancel_plane': ('_Cancel', [], ''),
    'cancel_three_point': ('_3Point w0,0,2 _Cancel', [], ''),
    'cancel_curve': ('_WorldXY', POINTS[:2], '_Cancel'),
}
DISCOVERY_NAMES = list(CASES)
CASES.update({
    'managed_xy_on': ('_WorldXY', [[p[0],p[1],0.] for p in POINTS], '_Enter'),
    'managed_xy_off': ('_WorldXY', POINTS, '_Enter'),
    'managed_two_on': ('_WorldXY', [[-4.,-3.,0.],[4.,3.,0.]], '_Enter'),
    'managed_close': ('_WorldXY', [[p[0],p[1],0.] for p in POINTS[:4]], '_Close'),
    'managed_bounded_xy_on': ('_WorldXY', [[p[0],p[1],0.] for p in POINTS], '_Enter'),
    'managed_bounded_xy_off': ('_WorldXY', POINTS, '_Enter'),
    'managed_bounded_two': ('_WorldXY', [[-4.,-3.,0.],[4.,3.,0.]], '_Enter'),
    'managed_bounded_close': ('_WorldXY', [[p[0],p[1],0.] for p in POINTS[:4]], '_Close'),
    'managed_bounded_vertical': ('_Vertical w0,0,0 w10,0,0', [[p[0],0.,p[1]] for p in POINTS], '_Enter'),
})


def request():
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=[dict(op='surface_interpolate_plane', id='interp_plane_' + k, case=k) for k in DISCOVERY_NAMES])

def managed_request():
    q=request();q['operations']=[dict(op='surface_interpolate_plane',id='interp_plane_'+k,case=k) for k in CASES if k.startswith('managed_') and 'bounded' not in k];return q

def bounded_request():
    q=request();q['operations']=[dict(op='surface_interpolate_plane',id='interp_plane_'+k,case=k) for k in BOUNDED_NAMES];return q


def validate(op):
    if not isinstance(op, dict) or set(op) != {'op', 'id', 'case'} or op['op'] != 'surface_interpolate_plane' or not isinstance(op.get('case'),str) or op['case'] not in CASES or op['id'] != 'interp_plane_' + op['case']:
        raise ValueError('fixed owned infinite-plane interpolation recipe required')

NEW = {
    'yz': ('_WorldYZ', [[0.,p[0],p[1]] for p in POINTS], '_Enter'),
    'zx': ('_WorldZX', [[p[0],0.,p[1]] for p in POINTS], '_Enter'),
    'through': ('_ThroughPoint w0,0,4', POINTS, '_Enter'),
    'three': ('_3Point w0,0,2 w10,0,2 w0,10,2', POINTS, '_Enter'),
    'oblique': ('_3Point w0,0,0 w10,0,5 w0,10,3', POINTS, '_Enter'),
    'object': ('_SelID @reference', POINTS, '_Enter'),
    'around': ('_AroundCurve _SelID @reference w3,0,4', [[3.,p[0],p[1]] for p in POINTS], '_Enter'),
    'named': ('_NamedCPlane InterpolationPlane', POINTS, '_Enter'),
    'cplane': ('_CPlaneXY', [[2.,p[0],p[1]] for p in POINTS], '_Enter'),
    'undo': ('_WorldXY', [[p[0],p[1],0.] for p in POINTS[:3]]+['_Undo']+[[p[0],p[1],0.] for p in POINTS[3:]], '_Enter'),
}
CASES.update({'managed_bounded_'+k:v for k,v in NEW.items()})
CASES.update({
    'control_bounded_cancel_plane': ('_Cancel', [], ''),
    'control_bounded_cancel_three': ('_3Point w0,0,2 _Cancel', [], ''),
    'control_bounded_cancel_curve': ('_WorldXY', [[-4.,-3.,0.],[4.,3.,0.]], '_Cancel'),
    'control_bounded_enter': ('_Enter', [], ''),
    'control_bounded_collinear': ('_3Point w0,0,2 w10,0,2 w20,0,2 _Cancel', [], ''),
    'managed_bounded_audit_far': ('_WorldXY', [[1000.*p[0],1000.*p[1],0.] for p in POINTS], '_Enter'),
    'managed_bounded_audit_relative_yz': ('_WorldYZ', [[0.,0.,0.],dict(relative=[2.,3.]),dict(relative=[-1.,2.]),dict(relative=[3.,-2.])], '_Enter'),
    'managed_bounded_audit_empty': ('_WorldXY', [[p[0],p[1],0.] for p in POINTS], '_Enter'),
})
BOUNDED_NAMES = ['managed_bounded_'+k for k in ['xy_on','xy_off','two','close','vertical']]
def workflow_request():
    q=request();names=BOUNDED_NAMES+['managed_bounded_'+k for k in NEW]+[k for k in CASES if k.startswith('control_bounded_')]
    q['operations']=[dict(op='surface_interpolate_plane',id='interp_plane_'+k,case=k) for k in names];return q

def audit_request():
    q=request();q['operations']=[dict(op='surface_interpolate_plane',id='interp_plane_'+k,case=k) for k in CASES if '_audit_' in k];return q

def metadata(case):
    origin,normal=[0.,0.,0.],[0.,0.,1.]
    suffix=case[len('managed_bounded_'):] if case.startswith('managed_bounded_') else case
    suffix={'world_yz':'yz','three_point':'three'}.get(suffix,suffix)
    picker=['IP','WorldXY']
    choices={
        'yz': ([0.,0.,0.],[1.,0.,0.],['IP','WorldYZ']),
        'audit_relative_yz': ([0.,0.,0.],[1.,0.,0.],['IP','WorldYZ']),
        'zx': ([0.,0.,0.],[0.,1.,0.],['IP','WorldZX']),
        'vertical': ([0.,0.,0.],[0.,1.,0.],['IP','Vertical','w0,0,0','w10,0,0']),
        'through': ([0.,0.,4.],normal,['IP','ThroughPoint','w0,0,4']),
        'three': ([0.,0.,2.],normal,['IP','3Point','w0,0,2','w10,0,2','w0,10,2']),
        'oblique': (origin,[-.5,-.3,1.],['IP','3Point','w0,0,0','w10,0,5','w0,10,3']),
        'object': ([0.,0.,4.],normal,['IP','Object','@reference']),
        'around': ([3.,0.,4.],[1.,0.,0.],['IP','AroundCurve','@reference','w3,0,4']),
        'named': ([0.,0.,6.],normal,['IP','NamedCPlane','InterpolationPlane']),
        'cplane': ([2.,3.,4.],[1.,0.,0.],['IP','CPlaneXY']),
    }
    if suffix in choices:origin,normal,picker=choices[suffix]
    controls={
        'cancel_plane':['IP','Cancel'],
        'cancel_three':['IP','3Point','w0,0,2','Cancel'],
        'cancel_curve':['IP','WorldXY','w-4,-3,0','w4,3,0','Cancel'],
        'enter':['IP',''],
        'collinear':['IP','3Point','w0,0,2','w10,0,2','w20,0,2','Cancel'],
    }
    if case.startswith('control_bounded_'):picker=controls[case[len('control_bounded_'):]]+['Cancel']
    return dict(plane=origin+normal,picker=picker,control=case.startswith('control_bounded_'))
