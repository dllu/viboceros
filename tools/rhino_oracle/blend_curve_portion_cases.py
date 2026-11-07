"""Prescribed temporary portions and original parent ownership for BlendCrv."""
def pilot(case,pick):
    first=['SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],[-1.,0.,0.]]
    second=['SubCrv',pick(1,[10.25,3.,0.]),[11.,3.,0.],[14.,3.,0.]]
    done=['Trim=No','Join=No','Enter']
    line=dict(type='line',start=[10.,3.,0.],end=[15.,3.,0.])
    circle=dict(type='circle',center=[0.,0.,0.],radius=5.,x_axis=[1.,0.,0.],normal=[0.,0.,1.])
    return {
        'portion_first':case(['BlendStart=CurveEnd']+first+[pick(1,[10.25,3.,0.])]+done),
        'portion_reverse':case(['BlendStart=CurveEnd','SubCrv',pick(0,[-.25,0.,0.]),[-1.,0.,0.],[-4.,0.,0.],pick(1,[10.25,3.,0.])]+done),
        'portion_second':case(['BlendStart=CurveEnd',pick(0,[-.25,0.,0.])]+second+done),
        'portion_join':case(['BlendStart=CurveEnd']+first+[pick(1,[10.25,3.,0.]),'Trim=Yes','Join=Yes','Enter']),
        'portion_circle':case(['BlendStart=CurveEnd','SubCrv',pick(0,[5.,0.,0.]),[5.,0.,0.],[0.,5.,0.],pick(1,[10.25,3.,0.])]+done,curves=[circle,line]),
        'portion_point':case(['BlendStart=CurveEnd']+first+['Point',[10.,3.,0.]]+done),
        'portion_cancel':case(['SubCrv','Cancel','Cancel'],pending_redo=True),
    }


def discovery(case,pick):
    if __package__:from .arc_blend_subcurve_cases import SUBCURVE_HOLDOUTS,SUBCURVE_POINTERS,SUBCURVE_CLOSED
    else:from arc_blend_subcurve_cases import SUBCURVE_HOLDOUTS,SUBCURVE_POINTERS,SUBCURVE_CLOSED
    names=['subcurve_cubic_join','subcurve_self_join','subcurve_mixed_self_join','subcurve_numeric','subcurve_negative_length','subcurve_zero','subcurve_locked','subcurve_cancel_end','subcurve_mouse_line','subcurve_mouse_arc','subcurve_mouse_circle','subcurve_length_units','subcurve_cancel_start','subcurve_circle_long','subcurve_circle_reverse_join','subcurve_circle_numeric_wrap','subcurve_circle_numeric_short']
    sources=dict(SUBCURVE_HOLDOUTS);sources.update(SUBCURVE_POINTERS);sources.update(SUBCURVE_CLOSED)
    result={}
    for name in names:
        old=sources[name]; actions=['BlendStart=CurveEnd']+old['actions'][:-1]
        if 'Join=Yes' not in actions:actions+=['Join=No']
        actions+=['Enter']
        result[name.replace('subcurve_','portion_')]=case(actions,curves=old['curves'],**{k:v for k,v in old.items()if k not in('actions','curves','measure_arcs','view')})
    first=['SubCrv',pick(0,[-.25,0.,0.]),[-4.,0.,0.],[-1.,0.,0.]]
    second=['SubCrv',pick(1,[10.25,3.,0.]),[11.,3.,0.],[14.,3.,0.]]
    result.update({
      'portion_both_join':case(['BlendStart=CurveEnd']+first+second+['Trim=Yes','Join=Yes','Enter']),
      'portion_point_join':case(['BlendStart=CurveEnd','Point',[10.,3.,0.]]+first+['Trim=Yes','Join=Yes','Enter']),
      'portion_pick_point':case(['BlendStart=PickPoint']+first+[pick(1,[12.,3.,0.]),'Trim=Yes','Join=No','Enter']),
    })
    return result
