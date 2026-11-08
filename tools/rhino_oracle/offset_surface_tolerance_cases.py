"""Prescribed tolerance getters, document defaults and remembered values."""
import copy
PLANE=dict(type='normal_surface',target=dict(kind='brep',
    points=[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],[0.,3.,0.]],
    aim=[2.,1.,0.],view='Top',flip=False))

BASE=['Corner=Sharp','Solid=Yes','DeleteInput=No','Solid=No','BothSides=No','Loose=No','Distance',.5]
def case(actions,**kwargs):
    value=dict(curves=[copy.deepcopy(PLANE)],view='Top',preselected=True,
        pending_redo=True,capture_undo=True,actions=BASE+actions)
    value.update(kwargs)
    return value

TOLERANCE_PILOT={
    'offset_tp_menu':case(['Tolerance',.001,'Enter']),
    'offset_tp_negative':case(['Tolerance',-.01,.002,'Enter']),
    'offset_tp_default':case(['Tolerance',0.,'Enter']),
    'offset_tp_document':case(['Enter'],absolute_tolerance=.003),
    'offset_tp_cancel':case(['Tolerance',.0001,'Cancel']),
    'offset_tp_remember':case(['Enter']),
    'offset_tp_getter_cancel':case(['Tolerance','Cancel','Cancel']),
    'offset_tp_getter_keep':case(['Tolerance','Enter','Enter']),
}

if __package__:
    from .offset_surface_loose_cases import BEZIER,RATIONAL
    from .offset_surface_loose_trim_cases import CURVED_TRIM
else:
    from offset_surface_loose_cases import BEZIER,RATIONAL
    from offset_surface_loose_trim_cases import CURVED_TRIM

TOLERANCE_HOLDOUTS={
    'offset_th_getter_keep':case(['Tolerance',.0001,'Tolerance','Enter','Enter']),
    'offset_th_default_option':case(['Tolerance',.001,'Tolerance','Default','Enter']),
    'offset_th_explicit_document':case(['Tolerance',.000001,'Enter']),
    'offset_th_explicit_new_document':case(['Enter'],absolute_tolerance=.003),
    'offset_th_inline':case(['Tolerance=.0002',.0003,'Enter']),
    'offset_th_loose_toggle':case(['Tolerance',.0002,'Loose=Yes','Loose=No','Enter']),
    'offset_th_units':case(['Tolerance',{'number':'.0001cm'},.0002,'Enter']),
    'offset_th_bezier_fine':case(['Tolerance',.000001,'Enter'],curves=[BEZIER]),
    'offset_th_bezier_coarse':case(['Tolerance',.001,'Enter'],curves=[BEZIER]),
    'offset_th_rational':case(['Tolerance',.0001,'Enter'],curves=[RATIONAL]),
    'offset_th_trimmed':case(['Tolerance',.0001,'Enter'],curves=[CURVED_TRIM]),
    'offset_th_bezier_solid':case(['Tolerance',.0001,'Solid=Yes','Enter'],curves=[BEZIER]),
    'offset_th_bezier_both':case(['Tolerance',.0001,'BothSides=Yes','Enter'],curves=[BEZIER]),
    'offset_th_getter_escape':case(['Tolerance','Cancel']),
    'offset_th_after_escape':case(['Enter']),
}

TOLERANCE_FINAL={
    'offset_tf_remember_set':case(['Tolerance',.0002,'Cancel']),
    'offset_tf_remember_document':case(['Enter'],absolute_tolerance=.003),
    'offset_tf_default_document':case(['Tolerance','Default','Enter'],absolute_tolerance=.003),
    'offset_tf_default_follow':case(['Enter'],absolute_tolerance=.004),
    'offset_tf_postselect':dict(case(['Tolerance',.0002,'Enter']),preselected=False,
        actions=[{'curve':0},'Enter']+BASE+['Tolerance',.0002,'Enter']),
    'offset_tf_postselect_cancel':dict(case(['Tolerance','Cancel']),preselected=False,
        actions=[{'curve':0},'Enter']+BASE+['Tolerance','Cancel']),
    'offset_tf_bezier_finer':case(['Tolerance',.0000001,'Enter'],curves=[BEZIER]),
}
