"""Owned closed and open SubD volume sources; fixed native command inputs."""
import math
if __package__:
    from .component_length_cases import source as edge_source
else:
    from component_length_cases import source as edge_source


def source(scene):
    if scene in ('open_cube','reversed_open','bent_grid'):
        if scene=='bent_grid':return edge_source(scene)
        result=edge_source('cube');result['faces'].pop(1)
        if scene=='reversed_open':result['faces']=[f[:1]+list(reversed(f[1:]))for f in result['faces']]
        return result
    if scene=='bipyramid7':
        points=[[.3,-.2,4.],[0.,0.,-2.]]
        for i in range(7):
            a=2*math.pi*i/7;points.append([3*math.cos(a),2.5*math.sin(a),.12*math.sin(3*a)])
        return dict(points=points,faces=[[0,2+i,2+(i+1)%7]for i in range(7)]+[[1,2+(i+1)%7,2+i]for i in range(7)])
    if scene in ('cube','skew_cube','creased_cube','sharp_cube','reversed_cube','translated_cube','scaled_cube'):
        result=edge_source('cube')
        if scene in ('skew_cube','translated_cube','scaled_cube'):
            result['points'][0]=[-.4,.2,-.1];result['points'][6]=[4.5,2.6,5.8]
        if scene=='translated_cube':result['points']=[[x+100,y-200,z+300]for x,y,z in result['points']]
        if scene=='scaled_cube':result['points']=[[2*x,2*y,2*z]for x,y,z in result['points']]
        if scene=='reversed_cube':result['faces']=[f[:1]+list(reversed(f[1:]))for f in result['faces']]
        if scene in ('creased_cube','sharp_cube'):
            result['creases']=[[0,1],[1,2],[2,3],[0,3]]
            if scene=='sharp_cube':
                result['creases']+= [[4,5],[5,6],[6,7],[4,7],[0,4],[1,5],[2,6],[3,7]]
                result['corners']=list(range(8))
        return result
    raise ValueError('owned SubD volume scene required')


def cases():
    scenarios=[dict(scene=scene,flat=flat,preselect=True,mixed=False)for scene in ('cube','skew_cube','creased_cube','sharp_cube','reversed_cube','translated_cube','scaled_cube')for flat in (False,True)]
    scenarios += [dict(scene='bipyramid7',flat=False,preselect=True,mixed=False),
                  dict(scene='skew_cube',flat=False,preselect=False,mixed=False),
                  dict(scene='skew_cube',flat=False,preselect=True,mixed=True),
                  dict(scene='reversed_cube',flat=False,preselect=True,mixed=True)]
    return [dict(op='subd_volume',id=s['scene']+'_'+str(int(s['flat']))+'_'+str(int(s['preselect']))+'_'+str(int(s['mixed']))+'_'+command,command=command,**s)for s in scenarios for command in ('Volume','VolumeCentroid')]


def validate(op):
    if (not isinstance(op,dict) or op not in cases()+open_cases()+admission_cases()
            or any(type(op.get(k))is not bool for k in ('flat','preselect','mixed'))
            or ('replace_reversed'in op and type(op['replace_reversed'])is not bool)):
        raise ValueError('fixed owned SubD volume case required')


def open_cases():
    scenarios=[('open_cube',False,True,False,'yes'),('open_cube',True,True,False,'yes'),
               ('open_cube',False,True,False,'no'),('open_cube',False,True,False,'escape'),
               ('open_cube',False,False,False,'yes'),('reversed_open',False,True,False,'yes'),
               ('bent_grid',False,True,False,'yes'),('bent_grid',False,True,True,'yes'),
               ('open_cube',False,True,True,'yes')]
    return [dict(op='subd_volume',id=scene+'_'+str(int(flat))+'_'+str(int(pre))+'_'+str(int(mixed))+'_'+command+'_'+choice,command=command,scene=scene,flat=flat,preselect=pre,mixed=mixed,open_confirmation=choice)for scene,flat,pre,mixed,choice in scenarios for command in ('Volume','VolumeCentroid')]


def admission_cases():
    return [dict(op='subd_volume',id=scene+'_replace_'+command,scene=scene,command=command,flat=False,preselect=True,mixed=False,replace_reversed=True,**(dict(open_confirmation='yes')if scene=='open_cube'else{}))for scene in ('cube','open_cube')for command in ('Volume','VolumeCentroid')]
