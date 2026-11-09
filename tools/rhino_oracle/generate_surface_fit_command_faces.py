"""Keep one NURBS face to distinguish selection rejection from fit failure."""
import copy
from .capture_family import capture
from .generate_surface_fit_commands import request as pilot
from .generate_surface_fit_adaptive import bezier


def request():
    q=pilot();operations=[]
    for operation in q['operations']:
        if operation['id'].endswith(('polynomial_null','polynomial_dense','degree_two','plane_copy')):
            op=copy.deepcopy(operation);op['id']+='-single-face';op['construction']='single_face_brep';operations.append(op)
    # Frozen capture label is historical: operations[0] is the plane, not polynomial.
    original=copy.deepcopy(operations[0]);original.update(id='fit-command-polynomial-loose-single-face',fit_tolerance=1.,delete_input=True)
    operations.append(original)
    for delete in [False,True]:
        op=copy.deepcopy(operations[0]);op.update(id='fit-command-rational-bezier-null-'+str(int(delete)),surface=bezier(True),delete_input=delete)
        operations.append(op)
    q['operations']=operations
    return q


if __name__=='__main__':
    capture('surface_fit_command_faces',request(),['generate_surface_fit_command_faces.py','generate_surface_fit_commands.py',
        'surface_fit_command_probe.py','generate_surface_fit_adaptive.py','generate_surface_knot_reduction.py',
        'generate_surface_knot_reduction_holdouts.py','structural_sdk.py','rhino_worker.py','merge_edges_probe.py','client.py'],
        'VibocerosOracleSurfaceFitCommandFaces20261108',timeout=1200)
