"""Independent characterization of the public surface-jet construction policy."""
import math
def dot(a,b):return sum(x*y for x,y in zip(a,b))
def scale(a,s):return [x*s for x in a]
def add(*values):return [sum(x)for x in zip(*values)]
def cross(a,b):return [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
def unit(a):return scale(a,1/math.sqrt(dot(a,a)))
def contacts(value):
    result=[];surface=False
    for index,action in enumerate(value['prescribed_case']['actions']):
        if action=='Edges':surface=True
        elif action=='Curves':surface=False
        elif isinstance(action,dict)and 'curve'in action:result.append((index,action['curve'],surface))
        elif action=='Point':result.append((index,None,False))
    return result
def angle(value,endpoint):
    prefix='AtAngle_'+str(endpoint+1)+'='
    return next((float(a[len(prefix):])for a in reversed(value['prescribed_case']['actions'])if isinstance(a,str)and a.startswith(prefix)),0.)
def frame(value,action):
    click=next(c for c in value['input_trace'][0]['clicks']if c['action']==action)
    candidates=sorted(click['edge_pick_context']['candidates'],key=lambda c:c['distance'])
    if not candidates or len(candidates)>1 and candidates[1]['distance']-candidates[0]['distance']<=1e-8:raise ValueError('ambiguous higher surface input')
    candidate=candidates[0];face=candidate['faces'][0]
    return candidate,face
def targets(partials,d):
    su,sv,uu,uv,vv,uuu,uuv,uvv,vvv=partials
    x=unit(su);n=unit(cross(su,sv));y=unit(cross(n,x))
    def solve(w):
        b=dot(w,y)/dot(sv,y)
        return [(dot(w,x)-b*dot(sv,x))/dot(su,x),b]
    a,b=solve(d)
    second=add(scale(uu,a*a),scale(uv,2*a*b),scale(vv,b*b))
    third=add(scale(uuu,a**3),scale(uuv,3*a*a*b),scale(uvv,3*a*b*b),scale(vvv,b**3))
    parameter_line=scale(n,dot(third,n)-3*dot(second,d)*dot(second,n))
    au,bv=solve(scale(second,-1))
    section=add(third,scale(uu,3*a*au),scale(uv,3*(a*bv+b*au)),scale(vv,3*b*bv))
    return parameter_line,scale(n,dot(section,n))
def characterize(rows):
    evidence=[]
    for key,value in rows.items():
        if not key.startswith('blend_curve_surface_high_discovery/'):continue
        output=next(o for o in value['after']if o['attributes']['name']=='')
        for endpoint,(action,_,surface)in enumerate(contacts(value)):
            if not surface:continue
            candidate,face=frame(value,action);normal=scale(face['normal'],-1. if face['reversed']else 1.)
            tangent=scale(candidate['tangent'],-1. if face['trims'][0]['reversed']^face['reversed']else 1.)
            theta=math.radians(angle(value,endpoint));d=unit(add(scale(unit(cross(tangent,normal)),math.cos(theta)),scale(tangent,math.sin(theta))))
            if endpoint:d=scale(d,-1.)
            third,section=targets(face['third_order']['partials'],d)
            derivatives=output['derivative_samples'][endpoint*2]['derivatives'];speed=math.sqrt(dot(derivatives[1],derivatives[1]));measured=scale(derivatives[3],1/speed**3)
            evidence.append(dict(id=key,endpoint=endpoint,measured=measured,parameter_line_normal_target=third,normal_section_target=section,parameter_line_error=math.dist(third,measured),normal_section_error=math.dist(section,measured)))
    if len(evidence)!=36 or max(e['parameter_line_error']for e in evidence)>1e-8 or max(e['normal_section_error']for e in evidence)<.05:raise ValueError('surface third-order policy evidence changed')
    return evidence
