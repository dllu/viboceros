"""Output-derived public meshing diagnostics, separate from command input evidence."""
from collections import Counter

def cycle(values):
    return min(tuple(values[i:]+values[:i]) for i in range(len(values)))


def cells(mesh):
    return sorted(cycle([tuple(mesh['vertices'][i]) for i in f]) for f in mesh['faces'])


def sdk_calibration(capture):
    """Compare public APIs only; their inputs are captured *outputs*, not picks."""
    from .insert_point_mesh_sdk_cases import CASES
    result = []
    base = capture('insert_point_mesh_sdk')['results']
    origins = capture('insert_point_mesh_sdk_origins')['results']
    for row, rotated in zip(base, origins):
        recipe = CASES[row['id']]
        if row['value']['input'] != recipe or rotated['value']['input'] != recipe:
            raise ValueError('public meshing recipe differs')
        source = next(r['value'] for r in capture(recipe['capture'])['results']
                      if r['id'] == recipe['source_id'])
        native = next(o['mesh'] for o in source['after'] if o['source'] == 0)
        target = dict(vertices=recipe['vertices'], faces=recipe['captured_faces'])
        if Counter(cells(target)) - Counter(cells(native)):
            raise ValueError('calibration region is not from the captured mesh')
        output = row['value']['results']
        matches = {name: cells(value.get('mesh', value)) == cells(target)
                   for name, value in output.items()}
        rotations = rotated['value']['origins']
        if set(rotations) != {str(i) for i in range(len(recipe['vertices']))}:
            raise ValueError('missing public meshing boundary origin')
        result.append(dict(id=row['id'], api_matches=matches,
                           matching_origins=[i for i in range(len(recipe['vertices']))
                                             if cells(rotations[str(i)]['quads']) == cells(target)]))
    return result


def methods_calibration(capture):
    """Record exact cell matches; float32 or extra-vertex outputs remain distinct."""
    from .insert_point_mesh_sdk_cases import CASES
    methods = {'tessellation_free', 'tessellation_boundary', 'lines_4', 'lines_6',
               'planar_default', 'planar_simple'}
    result = []
    for row in capture('insert_point_mesh_sdk_methods')['results']:
        recipe = CASES[row['id'].removesuffix('_methods')]
        value = row['value']
        if value['input'] != recipe or set(value['methods']) != methods:
            raise ValueError('public meshing method recipe differs')
        target = dict(vertices=recipe['vertices'], faces=recipe['captured_faces'])
        native = next(r['value'] for r in capture(recipe['capture'])['results']
                      if r['id'] == recipe['source_id'])
        mesh = next(o['mesh'] for o in native['after'] if o['source'] == 0)
        if Counter(cells(target)) - Counter(cells(mesh)):
            raise ValueError('method calibration is not a captured output region')
        matches, counts = {}, {}
        for name, output in value['methods'].items():
            if 'error' in output or set(output) not in ({'mesh', 'quads'}, {'mesh', 'quads', 'changed'}):
                raise ValueError('incomplete public meshing method')
            for kind in ['mesh', 'quads']:
                mesh = output[kind]
                key = name+'_'+kind
                matches[key] = mesh is not None and cells(mesh) == cells(target)
                counts[key] = len(mesh['vertices']) if mesh is not None else None
        result.append(dict(id=row['id'], api_matches=matches, vertex_counts=counts))
    return result


def spatial_calibration(capture):
    """Compare cell indices through a bounded coordinate bijection; no input parity claim."""
    import math
    from .insert_point_spatial_sdk_cases import CASES
    result=[]
    for row in capture('insert_point_mesh_sdk_spatial')['results']:
        case=CASES[row['id'].removesuffix('_spatial')];value=row['value']
        if value['input']!=case:raise ValueError('spatial calibration recipe differs')
        target=dict(vertices=case['vertices'],faces=case['captured_faces'])
        source=next(r['value']for r in capture(case['capture'])['results']if r['id']==case['source_id'])
        mesh=next(o['mesh']for o in source['after']if o['source']==0)
        if Counter(cells(target))-Counter(cells(mesh)):raise ValueError('spatial calibration is not an output region')
        wanted=sorted(cycle(f)for f in case['captured_faces']);n=len(case['vertices'])
        variants=value['variants']
        if set(variants)!={method+'_'+str(i)for method in ['closed','triangles']for i in range(n)}:
            raise ValueError('incomplete spatial boundary origin calibration')
        matching=[]
        for key,entry in variants.items():
            if 'error'in entry:raise ValueError('public spatial calibration failed')
            for kind in ['mesh','quads']:
                mesh=entry[kind]
                if mesh is None:continue
                mapping=[min(range(n),key=lambda i:math.dist(p,case['vertices'][i]))for p in mesh['vertices']]
                if (len(mapping)!=n or len(set(mapping))!=n or
                    any(math.dist(p,case['vertices'][i])>1e-6 for p,i in zip(mesh['vertices'],mapping))):
                    continue
                if sorted(cycle([mapping[i]for i in f])for f in mesh['faces'])==wanted:matching.append(key+'_'+kind)
        result.append(dict(id=row['id'],matching_cells=sorted(matching)))
    return result
