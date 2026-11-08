"""Authored physical edge-picking workflows for the MatchSrf command."""
import copy
if __package__:
    from .match_surface_cases import surface
else:
    from match_surface_cases import surface


def case(actions,**changes):
    value=dict(curves=[surface(),surface(True)],actions=actions,view='Top',
        preselected=False,selection_order=[0,1],common_group=False,pending_redo=True)
    value.update(changes);return copy.deepcopy(value)


def picks(first=.25,second=.25):
    return [dict(curve=0,edge=1,pick=[1.,first,.05+.19*first]),
            dict(curve=1,edge=3,pick=[1.12,second,.2+.15*second])]


PILOT={
    'match_cmd_default':case(picks()+['Enter']),
    'match_cmd_position':case(picks()+['Continuity=Position','Enter']),
    'match_cmd_cancel':case(picks()+['Cancel']),
}
RESET=['Mode=Curvature','PreserveOtherEnd=Position','MatchByClosestPoints=No',
       'RefineMatch=No','AverageSurfaces=No','IsocurveDirection=Automatic']
OPTIONS={
    'match_cmd_mode_position':case(picks()+RESET+['Mode=Position','Enter'],capture_recall=True),
    'match_cmd_mode_tangency':case(picks()+RESET+['Mode=Tangency','Enter']),
    'match_cmd_preserve_curvature':case(picks()+RESET+['PreserveOtherEnd=Curvature','Enter']),
    'match_cmd_opposite_pick_ends':case(picks(.25,.75)+RESET+['Enter']),
    'match_cmd_preselected':case(picks()+RESET+['Enter'],preselected=True),
    'match_cmd_option_menus':case(picks()+RESET+['Mode','Tangency','PreserveOtherEnd','Tangency',
                                'IsocurveDirection','Preserve','Enter']),
    'match_cmd_closest_refine':case(picks()+RESET+['MatchByClosestPoints=Yes','RefineMatch=Yes','Enter']),
    'match_cmd_tolerance_menu':case(picks()+RESET+['Tolerances','Cancel']),
    'match_cmd_cancel_options':case(picks()+RESET+['Mode=Position','PreserveOtherEnd=None','Cancel']),
    'match_cmd_after_cancel':case(picks()+['Enter']),
}
CASES=copy.deepcopy(dict(PILOT,**OPTIONS))


def validate(op):
    if (not isinstance(op,dict) or set(op)!={'op','id','case'} or op.get('op')!='match_surface_command'
            or op.get('id')!=op.get('case') or op.get('case') not in CASES):
        raise ValueError('invalid prescribed MatchSrf command case')


def request(names):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),
        operations=[dict(op='match_surface_command',id=name,case=name) for name in names])

QUALIFIED={}
for preserve in ['Nothing','Position','Tangency','Curvature']:
    QUALIFIED['match_cmd_high_preserve_'+preserve.lower()]=case(picks()+RESET+[
        'PreserveOtherEnd='+preserve,'Enter'],curves=[surface(degree=5),surface(True)])
QUALIFIED['match_cmd_low_preserve_nothing']=case(picks()+RESET+['Mode=Position','PreserveOtherEnd=Nothing','Enter'])
QUALIFIED['match_cmd_low_preserve_tangency']=case(picks()+RESET+['Mode=Tangency','PreserveOtherEnd=Tangency','Enter'])
for iso in ['Keep','MatchTarget','Perpendicular']:
    value=case(picks()+RESET+['Mode=Tangency','IsocurveDirection='+iso,'Enter'])
    for c in value['curves'][1]['control_points']:c['point'][0]+=.2*c['point'][1]
    # The target mouse aims follow the authored skew.
    value['actions'][1]['pick'][0]+=.05
    QUALIFIED['match_cmd_iso_'+iso.lower()]=value
QUALIFIED['match_cmd_tolerances_set']=case(picks()+RESET+['Tolerances','PositionTolerance=0.00001',
    'TangencyTolerance=0.001','CurvatureTolerance=0.1','Enter','Enter'])
QUALIFIED['match_cmd_cancel_first_pick']=case(picks()[:1]+['Cancel'],preselected=True)
QUALIFIED['match_cmd_cancel_no_picks']=case(['Cancel'],preselected=True)
CASES.update(copy.deepcopy(QUALIFIED))

TOLERANCE_GETTERS={
    'match_tol_first_cancel':case(picks()+RESET+['Tolerances','Cancel']),
    'match_tol_after_first_cancel':case(picks()+RESET+['Tolerances','Enter','Enter']),
    'match_tol_second_commit':case(picks()+RESET+['Tolerances','Enter','Enter']),
    'match_tol_bare_valid':case(picks()+RESET+['Tolerances','PositionTolerance',{'number':'0.00002'},
        'TangencyTolerance',{'number':'0.5'},'CurvatureTolerance',{'number':'0.02'},'Enter','Enter']),
    'match_tol_bare_empty':case(picks()+RESET+['Tolerances','PositionTolerance','Enter',
        'TangencyTolerance','Enter','CurvatureTolerance','Enter','Enter','Enter']),
    'match_tol_bad_then_valid':case(picks()+RESET+['Tolerances','PositionTolerance',{'number':'-1'},
        {'number':'0'},{'number':'0.00004'},'TangencyTolerance',{'number':'0.01'},{'number':'0.1'},
        'CurvatureTolerance',{'number':'-1'},{'number':'0'},{'number':'0.2'},'Enter','Enter']),
    'match_tol_cancel_pending':case(picks()+RESET+['Tolerances','PositionTolerance=0.00008',
        'CurvatureTolerance=0.3','TangencyTolerance','Cancel']),
    'match_tol_after_pending':case(picks()+RESET+['Tolerances','Enter','Enter'],absolute_tolerance=.0001),
    'match_tol_complete_menus':case(picks()+RESET+['Mode','Tangency','PreserveOtherEnd','Curvature',
        'IsocurveDirection','Keep','Enter']),
}
CASES.update(copy.deepcopy(TOLERANCE_GETTERS))
