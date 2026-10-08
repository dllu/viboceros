"""Derive public closest-point queries from the retained native cap sources."""
import json
from pathlib import Path


def request():
    source = Path(__file__).with_name('observations') / 'surface_rebuild_singular_trim.json'
    capture = json.loads(source.read_text())
    operations = []
    for case in ('sphere_cap_retrim', 'sphere_swapped_cap_retrim'):
        value = next(row['value'] for row in capture['results'] if row['value']['case'] == case)
        definition = value['command']['after_script'][-1]['brep']['faces'][0]['definition']
        surface = dict(
            degree_u=definition['degree'][0], degree_v=definition['degree'][1],
            control_point_count_u=definition['control_count'][0],
            control_point_count_v=definition['control_count'][1],
            knots_u=definition['knots_u'], knots_v=definition['knots_v'],
            control_points=definition['control_points'],
            domain_u=definition['domain_u'], domain_v=definition['domain_v'],
        )
        operations.append(dict(op='surface_closest_point', id=case, surface=surface,
                               points=value['before'][0]['brep']['edges'][0]['curve']['samples']))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-10, relative=1e-12, angular=1e-10),
                operations=operations)


if __name__ == '__main__':
    destination = Path(__file__).with_name('fixtures') / 'surface_rebuild_cap_projection.json'
    destination.write_text(json.dumps(request(), indent=2) + '\n')
