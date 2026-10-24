"""Coplanar coverage and transverse lines meeting in the same mesh pair."""
from copy import deepcopy
if __package__:
    from .mesh_boolean_command_cases import scene
else:
    from mesh_boolean_command_cases import scene


def combine(meshes):
    result=dict(kind='mesh',vertices=[],faces=[])
    for mesh in meshes:
        offset=len(result['vertices'])
        result['vertices'].extend(deepcopy(mesh['vertices']))
        result['faces'].extend([[offset+i for i in face]for face in mesh['faces']])
    return result


def cases(sheet, xy, islands):
    patch=sheet([[2.,1.,0.],[4.,1.,0.],[4.,3.,0.],[2.,3.,0.]])
    wall=lambda a,b:sheet([[a[0],a[1],-1.],[b[0],b[1],-1.],
        [b[0],b[1],1.],[a[0],a[1],1.]])
    result={}
    for name,a,b in [
        ('border',[2.,1.],[4.,1.]),('corner',[2.,1.],[2.,3.]),
        ('through',[3.,-1.],[3.,4.]),('vertex',[0.,1.],[2.,1.]),
        ('outside',[1.,-1.],[1.,4.]),
    ]:
        result[name]=dict(operation='intersect',preselect=False,
            sources=[deepcopy(xy),combine([patch,wall(a,b)])])
    result['two_regions']=dict(operation='intersect',preselect=False,
        sources=[deepcopy(xy),combine([islands,wall([0.,1.8],[4.,1.8])])])
    points=patch['vertices']
    result['curtain']=dict(operation='intersect',preselect=False,
        sources=[deepcopy(xy),combine([patch]+[wall(points[i],points[(i+1)%4])for i in range(4)])])
    result['face_touch']=scene('intersect','face_touch',preselect=False)
    opposite=deepcopy(xy)
    opposite['faces']=[list(reversed(face))for face in opposite['faces']]
    result['opposite_sheet']=dict(operation='intersect',preselect=False,sources=[deepcopy(xy),opposite])
    for value in result.values():
        value.update(arguments='_Tolerance=_Auto _CoplanarFaces=_Polylines',capture_intersection_sdk=True)
    return {'mesh_intersect_mixed_'+name:value for name,value in result.items()}
