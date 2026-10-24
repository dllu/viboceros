"""Fixed representation and coordinate variants of coincident mesh surfaces."""
from copy import deepcopy
from math import sqrt
if __package__:
    from .mesh_boolean_command_cases import scene
else:
    from mesh_boolean_command_cases import scene


def cases(triangulate, sheet, overlap):
    result = {}
    names = ['base', 'drop', 'triangles', 'face_order', 'vertex_order',
             'winding', 'one_winding', 'one_face_order', 'one_vertex_order',
             'translated', 'rotated', 'scaled']
    for name in names:
        value = scene('intersect', 'identical', preselect=False,
            arguments='_Tolerance=_Auto _CoplanarFaces=_'+('Drop' if name=='drop' else 'Polylines'))
        for index, source in enumerate(value['sources']):
            if name == 'triangles': value['sources'][index] = triangulate(source)
            elif name in ['face_order', 'one_face_order'] and (name=='face_order' or index==1):
                source['faces'].reverse()
            elif name in ['vertex_order', 'one_vertex_order'] and (name=='vertex_order' or index==1):
                count=len(source['vertices'])
                source['vertices'].reverse()
                source['faces']=[[count-1-i for i in face]for face in source['faces']]
            elif name in ['winding','one_winding'] and (name=='winding' or index==1):
                source['faces']=[list(reversed(face))for face in source['faces']]
            elif name == 'translated':
                source['vertices']=[[x+12.,y-7.,z+3.]for x,y,z in source['vertices']]
            elif name == 'rotated':
                c=sqrt(.5)
                source['vertices']=[[c*(x-y)+8.,c*(x+y)-4.,z]for x,y,z in source['vertices']]
            elif name == 'scaled':
                source['vertices']=[[.25*p for p in xyz]for xyz in source['vertices']]
        result[name]=value
    result['open_sheet']=dict(operation='intersect',preselect=False,
        sources=deepcopy([sheet,sheet]),arguments='_Tolerance=_Auto _CoplanarFaces=_Polylines')
    result['partial_area']=dict(operation='intersect',preselect=False,
        sources=deepcopy([sheet,overlap]),arguments='_Tolerance=_Auto _CoplanarFaces=_Polylines')
    result['crossing']=scene('intersect','overlap',preselect=False,
        arguments='_Tolerance=_Auto _CoplanarFaces=_Polylines')
    for value in result.values(): value['capture_intersection_sdk']=True
    return {'mesh_intersect_coincident_'+name:value for name,value in result.items()}
