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

TOLERANCE_DISCOVERY={
    'match_tol_menu_default':case(picks()+RESET+['Tolerances','Enter','Enter']),
    'match_tol_scalar_values':case(picks()+RESET+['Tolerances','PositionTolerance',{'number':'0.00001'},
        'TangencyTolerance',{'number':'0.001'},'CurvatureTolerance',{'number':'0.1'},'Enter','Enter']),
    'match_tol_ranges':case(picks()+RESET+['Tolerances','PositionTolerance=-1','PositionTolerance=0',
        'PositionTolerance=0.00002','TangencyTolerance=0.00001','TangencyTolerance=0',
        'CurvatureTolerance=-1','CurvatureTolerance=0','CurvatureTolerance=0.01','Enter','Enter']),
    'match_tol_cancel_scalar':case(picks()+RESET+['Tolerances','PositionTolerance',{'number':'0.00003'},'TangencyTolerance','Cancel']),
    'match_tol_after_cancel':case(picks()+RESET+['Tolerances','Enter','Enter']),
    'match_tol_new_document':case(picks()+RESET+['Tolerances','Enter','Enter'],absolute_tolerance=0.0001),
}
CASES.update(copy.deepcopy(TOLERANCE_DISCOVERY))

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

if __package__:
    from .match_surface_cases import FINAL as _SURFACE_FINAL
else:
    from match_surface_cases import FINAL as _SURFACE_FINAL

def rational(actions,**changes):
    value=case(actions,curves=_SURFACE_FINAL['match_f_rational_refine_curvature']['sources'],**changes)
    for a in value['actions']:
        if isinstance(a,dict) and 'pick' in a:a['pick'][0]+=.12*a['pick'][1]
    return value

TOLERANCE_MATCHES={
    'match_tol_refine_first_defaults':rational(picks()+RESET+['RefineMatch=Yes','Tolerances','Enter','Enter']),
    'match_tol_refine_document_defaults':rational(picks()+RESET+['RefineMatch=Yes','Tolerances','Enter','Enter']),
    'match_tol_refine_loose':rational(picks()+RESET+['RefineMatch=Yes','Tolerances','PositionTolerance=0.001',
        'TangencyTolerance=1','CurvatureTolerance=0.01','Enter','Enter']),
    'match_tol_refine_tighter':rational(picks()+RESET+['RefineMatch=Yes','Tolerances','PositionTolerance=0.00001',
        'TangencyTolerance=0.1','CurvatureTolerance=100','Enter','Enter']),
    'match_tol_refine_cancel':rational(picks()+RESET+['RefineMatch=Yes','Tolerances','PositionTolerance=0.001',
        'TangencyTolerance=1','CurvatureTolerance=0.001','Cancel']),
    'match_tol_refine_after_cancel':rational(picks()+['RefineMatch=Yes','Tolerances','Enter','Enter'],absolute_tolerance=.0001),
    'match_tol_fraction_values':case(picks()+RESET+['Tolerances','PositionTolerance',{'number':'1/1000'},
        'TangencyTolerance',{'number':'1/2'},'CurvatureTolerance',{'number':'1/20'},'Enter','Enter']),
    'match_tol_value_boundaries':case(picks()+RESET+['Tolerances','PositionTolerance=0.000009',
        'PositionTolerance=0.00001','TangencyTolerance=0.099999','TangencyTolerance=0.1',
        'CurvatureTolerance=0.00000001','CurvatureTolerance=0.00000001490116119384765625','Enter','Enter']),
    'match_tol_value_upper_bounds':case(picks()+RESET+['Tolerances','PositionTolerance=1000',
        'TangencyTolerance=181','TangencyTolerance=720','CurvatureTolerance=200','CurvatureTolerance=2000','Enter','Enter']),
    'match_tol_outside_menu':case(picks()+RESET+['PositionTolerance=0.00005','TangencyTolerance=0.2',
        'CurvatureTolerance=0.4','Tolerances','Enter','Enter']),
}
CASES.update(copy.deepcopy(TOLERANCE_MATCHES))

TOLERANCE_DEFAULTS={
    'match_tol_seed_document':rational(picks()+RESET+['RefineMatch=Yes','Tolerances','Enter','Enter']),
    'match_tol_changed_document':rational(picks()+RESET+['RefineMatch=Yes','Tolerances','Enter','Enter'],absolute_tolerance=.0001),
    'match_tol_changed_again':rational(picks()+RESET+['RefineMatch=Yes','Tolerances','Enter','Enter'],absolute_tolerance=.00001),
}
TOLERANCE_FIRST_CUSTOM={
    'match_tol_first_custom':rational(picks()+RESET+['RefineMatch=Yes','Tolerances','PositionTolerance=0.002',
        'TangencyTolerance=1.5','CurvatureTolerance=0.2','Enter','Enter']),
    'match_tol_first_custom_recall':rational(picks()+RESET+['RefineMatch=Yes','Tolerances','Enter','Enter'],absolute_tolerance=.0001),
}
CASES.update(copy.deepcopy(TOLERANCE_DEFAULTS));CASES.update(copy.deepcopy(TOLERANCE_FIRST_CUSTOM))

AVERAGE_WORKFLOWS={
    'match_average_position':case(picks()+RESET+['Mode=Position','AverageSurfaces=Yes','Enter'],preselected=True),
    'match_average_tangency':case(picks()+RESET+['Mode=Tangency','AverageSurfaces=Yes','Enter'],common_group=True),
    'match_average_opposite_position':case(picks(.25,.75)+RESET+['Mode=Position','AverageSurfaces=Yes','Enter']),
    'match_average_option_menus':case(picks(.25,.75)+RESET+['Mode','Tangency','AverageSurfaces','Yes','IsocurveDirection','Keep','Enter']),
    'match_average_cancel':case(picks()+RESET+['Mode=Position','AverageSurfaces=Yes','Cancel']),
    'match_average_after_cancel':case(picks()+['Mode=Tangency','Enter']),
    'match_average_switch_single':case(picks()+['Mode=Position','AverageSurfaces=No','Enter']),
    'match_average_curvature_pending':case(picks()+RESET+['AverageSurfaces=Yes','Enter']),
}
CASES.update(copy.deepcopy(AVERAGE_WORKFLOWS))
