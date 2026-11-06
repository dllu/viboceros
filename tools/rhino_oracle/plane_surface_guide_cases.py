"""Guide selection and cancellation phases, with persistent preselection controls."""
GUIDE_CASES = {
    'guide_cancel_before_pick': dict(actions=['AroundCurve', 'Cancel'], guide=[[0,0,0],[0,0,10]], skip_guide_pick=True),
    'guide_cancel_after_pick': dict(actions=['AroundCurve', 'Cancel'], guide=[[0,0,0],[0,0,10]]),
    'guide_cancel_after_center': dict(actions=['AroundCurve',[0,0,5],'Cancel'], guide=[[0,0,0],[0,0,10]]),
    'guide_preselected': dict(actions=['AroundCurve',[0,0,5],8,5], guide=[[0,0,0],[0,0,10]], selected=False, guide_preselected=True, skip_guide_pick=True),
    'guide_preselected_cancel': dict(actions=['AroundCurve',[0,0,5],'Cancel'], guide=[[0,0,0],[0,0,10]], selected=False, guide_preselected=True, skip_guide_pick=True),
}
