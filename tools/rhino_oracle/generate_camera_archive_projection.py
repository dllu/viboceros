"""Supplement skew locked-camera archives with native points in front of the camera."""
from .generate_camera_archive import request as archive_request, capture

FAMILY = 'camera_archive_projection'


def request():
    query = archive_request()
    query['operations'] = [op for op in query['operations']
                           if op['projection'] == 'TwoPointPerspective' and op['rotate']
                           and '-orthogonal-' not in op['id']]
    for op in query['operations']:
        op['id'] = 'projection-' + op['id']
        op['camera_samples'] = True
    return query


def main():
    capture(FAMILY, request(), ['generate_camera_archive_projection.py'])


if __name__ == '__main__':
    main()
