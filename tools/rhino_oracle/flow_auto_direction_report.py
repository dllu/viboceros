"""Measure automatic Flow direction invariants; optional NumPy diagnostics.

Fitted native directions explain the captured frames. They do not select an
orientation from authored geometry and never initialize production morphs.
"""
import argparse
import json
import math
from pathlib import Path

import numpy as np

from .flow_probe import validate

BASE = Path(__file__).parent
ROTATIONS = {
    'rotate-x': [[1, 0, 0], [0, 0, -1], [0, 1, 0]],
    'rotate-y': [[0, 0, 1], [0, 1, 0], [-1, 0, 0]],
    'rotate-z': [[0, -1, 0], [1, 0, 0], [0, 0, 1]],
    'cycle': [[0, 0, 1], [1, 0, 0], [0, 1, 0]],
}


def array(value):
    a = np.asarray(value, dtype=float)
    if not np.isfinite(a).all():
        raise ValueError('nonfinite automatic-direction observation')
    return a


def unit(value):
    value = array(value)
    length = np.linalg.norm(value, axis=-1, keepdims=True)
    if np.any(length <= 0):
        raise ValueError('zero automatic-direction vector')
    return value / length


def direction_error(a, b):
    # A shared pole and its opposite produce the same point map.
    return float(np.linalg.norm(np.cross(unit(a), unit(b))))


def fit_direction(y, z, source_y, source_z):
    tangent = np.cross(y, z)
    for basis in (y, z, tangent):
        if np.max(abs(np.linalg.norm(basis, axis=1) - 1)) > 1e-10:
            raise ValueError('native automatic basis is not orthonormal')
    if np.max(abs(np.sum(y*z, axis=1))) > 1e-10:
        raise ValueError('native automatic basis is not orthogonal')
    indices = np.unique(np.linspace(0, len(y)-1, 9, dtype=int))
    design = np.c_[np.cross(tangent[indices], y[indices]), y[indices]]
    _, singular, vectors = np.linalg.svd(design, full_matrices=False)
    if len(singular) != 6 or singular[-2] < 1e-10:
        raise ValueError('insufficient sections for a unique automatic pole')
    k, b = vectors[-1, :3], vectors[-1, 3:]
    # A source axis can coincide with the projected pole, making b zero.
    # Either parallel coefficient recovers the pole; use the stronger one.
    pole = unit(k if np.linalg.norm(k) > np.linalg.norm(b) else b)
    # Treat numerically tied dominant coordinates consistently (e.g. a
    # 45-degree plane normal). The pole's sign has no geometric meaning.
    dominant = np.flatnonzero(abs(pole) >= max(abs(pole))-64*np.finfo(float).eps)[0]
    if pole[dominant] < 0:
        pole = -pole
    source_t = np.cross(source_y, source_z)
    sy = unit(np.cross(pole, source_t))
    sz = np.cross(source_t, sy)
    ty = unit(np.cross(pole, tangent))
    tz = np.cross(tangent, ty)
    yy = np.dot(source_y, sy)*ty + np.dot(source_y, sz)*tz
    zz = np.dot(source_z, sy)*ty + np.dot(source_z, sz)*tz
    return dict(
        fitted_direction=pole.tolist(),
        training_sections=len(indices),
        held_out_sections=len(y)-len(indices),
        checked_sections=len(y),
        max_shared_direction_basis_error=float(max(np.max(abs(yy-y)), np.max(abs(zz-z)))),
    )


def comparison(request, local, native):
    if request.get('protocol_version') != 1 or any(r.get('protocol_version') != 1 for r in (local, native)):
        raise ValueError('automatic-direction protocol differs')
    if local.get('engine') != 'viboceros' or native.get('engine') != 'rhino':
        raise ValueError('automatic-direction engines differ')
    operations = request['operations']
    ids = [o['id'] for o in operations]
    if len(set(ids)) != len(ids) or any(ids != [r['id'] for r in response['results']] for response in (local, native)):
        raise ValueError('automatic-direction identities differ')
    rows = []
    for op, a, n in zip(operations, local['results'], native['results']):
        validate(op)
        if op['base']['type'] != 'line' or op.get('command') or op['options'].get('reverse_base'):
            raise ValueError('automatic-direction witness requires a forward source line and SDK map')
        points = array(op['points'])
        actual = array(a['value']['sdk_points'])
        expected = array(n['value']['sdk_points'])
        if points.shape != (51, 3) or actual.shape != points.shape or expected.shape != points.shape:
            raise ValueError('automatic-direction witness requires 17 complete bases')
        start, end = array(op['base']['start']), array(op['base']['end'])
        centers = start + np.linspace(0, 1, 17)[:, None]*(end-start)
        scale = np.linalg.norm(points[1]-points[0])
        if not math.isfinite(scale) or scale <= 0:
            raise ValueError('invalid automatic probe size')
        source_y, source_z = (points[1::3]-points[::3])/scale, (points[2::3]-points[::3])/scale
        if (np.max(abs(points[::3]-centers)) > 1e-12
                or np.max(abs(source_y-source_y[0])) > 1e-12
                or np.max(abs(source_z-source_z[0])) > 1e-12
                or abs(np.linalg.norm(source_y[0])-1) > 1e-12
                or abs(np.linalg.norm(source_z[0])-1) > 1e-12
                or np.linalg.norm(np.cross(source_y[0], source_z[0])-unit(end-start)) > 1e-12):
            raise ValueError('authored automatic witness basis differs')
        y, z = (expected[1::3]-expected[::3])/scale, (expected[2::3]-expected[::3])/scale
        fit = fit_direction(y, z, source_y[0], source_z[0])
        local_y, local_z = (actual[1::3]-actual[::3])/scale, (actual[2::3]-actual[::3])/scale
        classification = n['value']['diagnostics']['target']['tolerance_classification']
        plane = next(row['plane'] for row in classification if row['tolerance'] == .1)
        rows.append(dict(
            id=op['id'], probe_size=float(scale), **fit,
            kernel_center_max_coordinate_error=float(np.max(abs(actual[::3]-expected[::3]))),
            kernel_basis_max_coordinate_error=float(max(np.max(abs(local_y-y)), np.max(abs(local_z-z)))),
            public_plane_at_0_1=plane,
            direction_error_from_public_plane=None if plane is None else direction_error(fit['fitted_direction'], plane['z']),
        ))
    by_id = {r['id']: r for r in rows}
    baseline = by_id['baseline']['fitted_direction']
    invariants, scales, rotations = [], [], []
    for row in rows:
        label = row['id']
        delta = direction_error(row['fitted_direction'], baseline)
        if label.startswith(('base-length-', 'base-shift-', 'target-shift-', 'both-shift-')):
            invariants.append(dict(id=label, direction_error=delta))
        elif '-scale-' in label:
            scales.append(dict(id=label, direction_error_from_baseline=delta,
                               public_plane_at_0_1_available=row['public_plane_at_0_1'] is not None))
        elif label.endswith('-both') and label.rsplit('-', 1)[0] in ROTATIONS:
            name = label.rsplit('-', 1)[0]
            matrix = array(ROTATIONS[name])
            back = array(row['fitted_direction']) @ matrix
            target_only = by_id[name+'-target']['fitted_direction']
            rotations.append(dict(id=name, joint_rotation_direction_error=direction_error(back, baseline),
                                  changed_source_orientation_direction_error=direction_error(row['fitted_direction'], target_only)))
    return dict(
        cases=len(rows), native_basis_sections=sum(r['checked_sections'] for r in rows),
        max_shared_direction_basis_error=max(r['max_shared_direction_basis_error'] for r in rows),
        max_kernel_center_coordinate_error=max(r['kernel_center_max_coordinate_error'] for r in rows),
        max_translation_and_source_length_direction_error=max(r['direction_error'] for r in invariants),
        target_reversal_direction_error=direction_error(by_id['target-reversed']['fitted_direction'], baseline),
        exact_refinement_direction_error=direction_error(by_id['refined']['fitted_direction'], baseline),
        production_kernel_changed=False, automatic_spatial_flow_qualified=False,
        fitted_directions_used_as_kernel_inputs=False,
        translation_and_source_length_invariants=invariants,
        scale_classification=scales, rotation_comparisons=rotations, results=rows,
    )


def report(base=BASE):
    def read(path):
        return json.loads((base/path).read_text())
    return comparison(read('fixtures/flow_auto_directions.json'),
                      read('observations/flow_auto_directions_viboceros_20261017.json'),
                      read('observations/flow_auto_directions_rhino_20261017.json'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.write_text(json.dumps(report(), indent=2, allow_nan=False)+'\n')


if __name__ == '__main__':
    main()
