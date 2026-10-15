"""Selected B-rep public SDK overloads; mesh getter capture is still pending."""
import json
from pathlib import Path
try:
    from .coplanar_faces_probe import validate as validate_recipe, run_brep
except ImportError:
    from coplanar_faces_probe import validate as validate_recipe, run_brep


def request():
    return json.loads(Path(__file__).with_name('fixtures').joinpath('selected_coplanar_faces.json').read_text())


def validate(operation):
    if (not isinstance(operation, dict)
            or set(operation) != {'op', 'id', 'source', 'faces', 'region'}
            or operation['op'] != 'selected_coplanar_faces'
            or type(operation['region']) is not bool):
        raise ValueError('invalid selected coplanar fields')
    validate_recipe(dict(op='merge_coplanar_faces', id=operation['id'], source=operation['source']))
    faces = operation['faces']
    if (not isinstance(faces, list) or len(faces) != (1 if operation['region'] else 2)
            or any(type(i) is not int or not 0 <= i < len(operation['source']['faces']) for i in faces)
            or len(set(faces)) != len(faces)):
        raise ValueError('invalid selected face indices')


def run(operation, host, iterations, tolerance):
    validate(operation)
    if operation['source']['type'] != 'mesh_brep':
        raise ValueError('selected mesh native getter capture remains pending')
    source = host['_object_source'](operation['source'], tolerance)
    try:
        # The one-index overload grows a connected planar region; the two-index
        # overload merges only that pair. Never substitute a whole-body merge.
        return run_brep(source, host, iterations, tolerance, operation['faces'])
    finally:
        if source is not None: source.Dispose()
