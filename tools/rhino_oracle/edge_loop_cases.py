"""Fixed planar loop controls: boundaries, creases, extraordinary and cyclic nets."""
import math

def discovery_cases():
    return [dict(op='edge_loop_command',id=kind+'_interior',kind=kind,scene='grid',pair=[6,7]) for kind in ('mesh','subd')]

def cases():
    rows=[dict(op='edge_loop_command',id=kind+'_'+name,kind=kind,scene='grid',pair=pair)
          for kind in ('mesh','subd') for name,pair in [('boundary',[1,2]),('corner',[0,1]),('vertical',[6,11])]]
    rows += [dict(op='edge_loop_command',id='mesh_no_uv_'+name,kind='mesh',scene='grid',pair=pair,uv=False)
             for name,pair in [('interior',[6,7]),('boundary',[1,2])]]
    rows += [dict(op='edge_loop_command',id=scene,kind='subd',scene=scene,pair=pair)
             for scene,pair in [('grid_smooth',[0,1]),('grid_crease',[6,7]),('grid_crease_end',[7,8]),('grid_corner',[6,7])]]
    rows += [dict(op='edge_loop_command',id=kind+'_'+scene,kind=kind,scene=scene,pair=[0,1])
             for kind in ('mesh','subd') for scene in ('fan3','fan5','fan5crease')]
    rows += [dict(op='edge_loop_command',id=kind+'_annulus_'+name,kind=kind,scene='annulus',pair=pair)
             for kind in ('mesh','subd') for name,pair in [('middle',[8,9]),('boundary',[0,1]),('radial',[1,9])]]
    rows += [dict(op='edge_loop_command',id=kind+'_triangles',kind=kind,scene='triangles',pair=[6,7]) for kind in ('mesh','subd')]
    return rows

def source(scene):
    if scene.startswith('grid') or scene=='triangles':
        result=dict(points=[[3.*x,3.*y,0.] for y in range(4) for x in range(5)],
                    faces=[[5*y+x,5*y+x+1,5*(y+1)+x+1,5*(y+1)+x] for y in range(3) for x in range(4)],corners=[0,4,15,19])
        if scene=='grid_smooth':result['corners']=[]
        if scene=='grid_crease':result['creases']=[[i,i+1] for i in range(5,9)]
        if scene=='grid_crease_end':result['creases']=[[6,7]]
        if scene=='grid_corner':result['corners'].append(7)
        if scene=='triangles':result['faces']=[t for a,b,c,d in result['faces'] for t in ([a,b,c],[a,c,d])]
        return result
    if scene.startswith('fan'):
        n=3 if scene=='fan3' else 5
        points=[[0.,0.,0.]]+[[r*math.cos(2*math.pi*(i+offset)/n),r*math.sin(2*math.pi*(i+offset)/n),0.]
                            for r,offset in ((3.,0.),(6.,.5)) for i in range(n)]
        result=dict(points=points,faces=[[0,i+1,n+i+1,(i+1)%n+1] for i in range(n)])
        if scene=='fan5crease':result['creases']=[[0,1],[0,3]]
        return result
    if scene=='annulus':
        return dict(points=[[r*math.cos(2*math.pi*i/8),r*math.sin(2*math.pi*i/8),0.] for r in (3.,6.,9.) for i in range(8)],
                    faces=[[j*8+i,(j+1)*8+i,(j+1)*8+(i+1)%8,j*8+(i+1)%8] for j in range(2) for i in range(8)])
    raise ValueError('unknown owned loop scene')

def validate(op):
    if not isinstance(op,dict) or op not in discovery_cases()+cases() or any(type(i) is not int for i in op.get('pair',[])) or ('uv' in op and type(op['uv']) is not bool):
        raise ValueError('fixed owned edge-loop command required')
