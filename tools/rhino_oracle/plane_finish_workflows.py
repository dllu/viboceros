"""Complete Plane prompt recipes; all planned tokens should be consumed."""
A, B, P = [1,2,3], [4,6,8], [7,-1,3]
V, H = [5,8,9], [1,2,10]
GUIDE = [[0,0,0],[0,0,10]]
FINISH_WORKFLOWS = {}

for name, actions in [
    ('initial_enter', ['Enter']),
    ('corner_enter', [A,'Enter']),
    ('center_initial_enter', ['Center','Enter']),
    ('center_enter', ['Center',A,'Enter']),
    ('three_initial_enter', ['3Point','Enter']),
    ('three_edge_enter', ['3Point',A,'Enter']),
    ('three_length_direction_enter', ['3Point',A,8,'Enter']),
    ('vertical_edge_enter', ['Vertical',A,'Enter']),
    ('vertical_length_direction_enter', ['Vertical',A,8,'Enter']),
    ('three_choose_enter', ['3Point',A,B,'Enter','Enter']),
    ('vertical_choose_enter', ['Vertical',A,V,'Enter','Enter']),
    ('three_width_choose_enter', ['3Point',A,B,5,'Enter']),
    ('midpoint_choose_enter', ['3Point','EdgeMidpoint',A,B,'Enter','Enter']),
    ('center_three_choose_enter', ['Center',A,'3Point',B,'Enter','Enter']),
]:
    FINISH_WORKFLOWS['complete_'+name] = dict(actions=actions, expected_success=False)

for name, actions in [
    ('length_enter', [A,8,'Enter']),
    ('center_length_enter', ['Center',A,8,'Enter']),
    ('three_default', ['3Point',A,B,'Enter',P]),
    ('vertical_default', ['Vertical',A,V,'Enter',H]),
    ('midpoint_default', ['3Point','EdgeMidpoint',A,B,'Enter',P]),
    ('center_three_default', ['Center',A,'3Point',B,'Enter',P]),
    ('center_three_length_width', ['Center',A,'3Point',8,B,5,P]),
    ('center_three_width', ['Center',A,'3Point',B,5,P]),
    ('midpoint_width', ['3Point','EdgeMidpoint',A,B,5,P]),
    ('three_length_default', ['3Point',A,8,B,'Enter',P]),
    ('center_three_length_default', ['Center',A,'3Point',8,B,'Enter',P]),
    ('midpoint_length_width', ['3Point','EdgeMidpoint',A,8,B,5,P]),
    ('late_three_length_default', [A,'3Point',8,B,'Enter',P]),
]:
    FINISH_WORKFLOWS['complete_'+name] = dict(actions=actions, expected_success=True)

for preselected in [False, True]:
    setup = dict(guide=GUIDE, guide_preselected=preselected,
                 skip_guide_pick=preselected, selected=not preselected)
    suffix = '_preselected' if preselected else '_picked'
    FINISH_WORKFLOWS['complete_guide_enter'+suffix] = dict(setup, actions=['AroundCurve','Enter'], expected_success=False)
    FINISH_WORKFLOWS['complete_guide_length_enter'+suffix] = dict(setup, actions=['AroundCurve',[0,0,5],8,'Enter'], expected_success=True)
FINISH_WORKFLOWS['complete_guide_center_enter'] = dict(guide=GUIDE,
    actions=['AroundCurve',[0,0,5],'Enter'], expected_success=False)

for suffix, value in [('yes','Yes'),('no','No'),('ignored','Ignored'),('empty','')]:
    FINISH_WORKFLOWS['complete_deform_'+suffix] = dict(actions=[
        'Deformable='+value,'UDegree=3','VDegree=2','UPointCount=6','VPointCount=5',A,[9,7,11]], expected_success=True)
FINISH_WORKFLOWS['complete_deform_repeated'] = dict(actions=[
    'Deformable','Deformable=No','UDegree=3','VDegree=2','UPointCount=6','VPointCount=5',A,[9,7,11]], expected_success=True, recovered=True)
for case in FINISH_WORKFLOWS.values():
    case['paced'] = True
