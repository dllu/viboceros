"""Characterize Alt's measured direction-only edit independently of Rust."""
import math
from .blend_curve_evidence import output
from .blend_curve_surface_policy import frame,dot,cross,unit,scale,add
def handles(value):
    p=[p['point']for p in output(value)['curve']['control_points']]
    return [[b-a for a,b in zip(p[0],p[1])],[b-a for a,b in zip(p[-1],p[-2])]]
def characterize(rows):
    get=lambda name:rows['blend_curve_alt_verification/'+name]
    pairs=[('verified_alt_target_pointer','verified_alt_baseline'),('verified_alt_both_pointer','verified_alt_select_pointer'),('alt_off_plane','verified_alt_select_typed')]
    equal=[]
    for a,b in pairs:
        x,y=output(get(a))['curve'],output(get(b))['curve']
        error=max(math.dist(p['point'],q['point'])for p,q in zip(x['control_points'],y['control_points']))
        if x['degree']!=y['degree']or len(x['control_points'])!=len(y['control_points'])or error>1e-10:raise ValueError('Alt selection policy changed')
        equal.append(dict(first=a,second=b,coefficient_error=error))
    base=handles(get('verified_alt_select_typed'));unequal=handles(get('alt_shift_unequal'));after=handles(get('alt_after_length'))
    lengths=[math.hypot(*h)for h in base+unequal+after]
    if max(abs(lengths[0]-lengths[1]),abs(lengths[0]-lengths[2]),abs(lengths[3]-5.),abs(lengths[4]-4.),abs(lengths[5]-lengths[1]))>1e-10:raise ValueError('Alt handle-length preservation changed')
    numeric=[]
    for family,name,endpoint,degrees,action in [('blend_curve_alt_verification','alt_numeric_angle',0,30.,3),('blend_curve_alt_followups','alt_numeric_negative',0,-35.,3),('blend_curve_alt_followups','alt_numeric_second',1,25.,4)]:
        v=rows[family+'/'+name];candidate,face=frame(v,action);n=scale(face['normal'],-1. if face['reversed']else 1.)
        tangent=scale(candidate['tangent'],-1. if face['trims'][0]['reversed']^face['reversed']else 1.)
        theta=math.radians(degrees);expected=add(scale(unit(cross(tangent,n)),math.cos(theta)),scale(tangent,math.sin(theta)))
        error=math.dist(unit(handles(v)[endpoint]),expected)
        if error>1e-10:raise ValueError('Alt numeric degree policy changed')
        numeric.append(dict(id=family+'/'+name,endpoint=endpoint,degrees=degrees,direction_error=error))
    contact=get('alt_preexisting_angle');contact_curve=output(contact)['curve'];reference=get('alt_numeric_angle')
    preserved=math.dist(unit(handles(contact)[0]),unit(handles(reference)[0]))
    moved_to_corner=contact_curve['control_points'][0]['point']==[0.,0.,0.]
    fixed=rows['blend_curve_alt_followups/alt_existing_angle_corrected']
    error=max(math.dist(p['point'],q['point'])for p,q in zip(output(fixed)['curve']['control_points'],output(get('verified_alt_select_typed'))['curve']['control_points']))
    if not moved_to_corner or max(error,preserved)>1e-10:raise ValueError('rounded contact/shape selection policy changed')
    ordinary=rows['blend_curve_alt_selection/plain_alt_far_ordinary'];ordinary_alt=rows['blend_curve_alt_picking/alt_far_ordinary']
    ordinary_error=max(math.dist(p['point'],q['point'])for p,q in zip(output(ordinary)['curve']['control_points'],output(ordinary_alt)['curve']['control_points']))
    secondary=rows['blend_curve_alt_selection/plain_alt_far_surface'];o=output(secondary);derivatives=o['derivative_samples'][-1]['derivatives']
    acceleration=dot(derivatives[2],unit(derivatives[1]))/dot(derivatives[1],derivatives[1])
    if ordinary_error>1e-10 or abs(acceleration)<.01 or abs(math.hypot(*handles(secondary)[1])-lengths[1])>1e-10 or o['curve']['control_points'][3]['point'][0]!=6.:raise ValueError('secondary control diagnostic changed')
    return dict(equivalent_outputs=equal,handle_lengths=dict(default=lengths[:2],shift_with_unequal_peer=lengths[2:4],after_length_edit=lengths[4:]),numeric_angles=numeric,rounded_pick_moves_contact_to_corner=moved_to_corner,rounded_contact_preserved_angle_error=preserved,corrected_shape_pick_error=error,plain_ordinary_pick_error=ordinary_error,secondary_shape_control=dict(id='blend_curve_alt_selection/plain_alt_far_surface',control=3,shape_edit_implemented=False,native_normalized_tangential_acceleration=acceleration))
