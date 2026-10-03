"""Capture actual interactive transform macros in a private settings scheme."""
import argparse
import json
from .client import OracleClient, _validate_response, load_request
from .transform_copy_probe import validate


def validate_request(request):
    if (not isinstance(request, dict) or type(request.get('protocol_version')) is not int or request['protocol_version'] != 1
            or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1
            or not isinstance(request.get('operations'), list) or not 1 <= len(request['operations']) <= 128):
        raise ValueError('transform Copy capture requires protocol 1, one iteration, and bounded cases')
    seen = set()
    for operation in request['operations']:
        validate(operation)
        if operation['id'] in seen:
            raise ValueError('duplicate transform case id')
        seen.add(operation['id'])


def capture(request, client, timeout=300):
    if client.settings_scheme is None:
        raise ValueError('transform Copy capture requires a private scheme')
    validate_request(request)
    observed = client.run_rhino(request, timeout)
    _validate_response(observed, 'rhino')
    if observed['iterations'] != 1 or [row['id'] for row in observed['results']] != [op['id'] for op in request['operations']]:
        raise ValueError('incomplete transform Copy observations')
    for operation, row in zip(request['operations'], observed['results']):
        value = row['value']
        if (not isinstance(value, dict)
                or set(value) != ({'before', 'after', 'last', 'undo', 'redo', 'succeeded', 'history', 'events', 'group_names'} | ({'target'} if 'mirror_target' in operation or 'normal_target' in operation else set()) | ({'frame'} if 'mouse_target' in operation or 'normal_target' in operation else set()) | ({'base_frame'} if 'NormalBase' in operation['inputs'] else set()))
                or type(value['succeeded']) is not bool or not isinstance(value['history'], str)
                or not isinstance(value['events'], list) or not isinstance(value['group_names'], list)
                or any(not isinstance(name, str) for name in value['group_names'])):
            raise ValueError('invalid transform Copy observation')
        if 'mouse_target' in operation or 'normal_target' in operation:
            from .translation_input import validate_frame
            validate_frame(value['frame'])
        if 'NormalBase' in operation['inputs']:
            base = value['base_frame']
            if not isinstance(base,dict) or set(base) != {'frame','camera'} or not isinstance(base['camera'],dict):
                raise ValueError('invalid Move Normal base calibration')
            validate_frame(base['frame'])
            from .move_normal_probe import validate_camera
            validate_camera(base['camera'],base['frame'])
        if 'mirror_target' in operation or 'normal_target' in operation:
            target = value['target']
            fields = {'type', 'selected', 'bounds', 'name', 'groups', 'layer', 'color_source', 'color'}
            fields |= {'vertices', 'faces'} if operation.get('mirror_target',{}).get('kind') == 'mesh' else {'definition'}
            if (not isinstance(target, dict) or set(target) != {'before', 'after'}
                    or not isinstance(target['before'], dict) or not isinstance(target['after'], dict)
                    or set(target['before']) != fields or set(target['after']) != fields
                    or {k:v for k,v in target['after'].items() if k != 'bounds'} != {k:v for k,v in target['before'].items() if k != 'bounds'}
                    or target['before'].get('selected') is not False
                    or ('definition' in fields and (not isinstance(target['before']['definition'],dict) or not target['before']['definition']))):
                raise ValueError('invalid transform reference observation')
            if 'normal_target' in operation:
                from .transform_copy_probe import finite
                bounds = [target[phase]['bounds'] for phase in ('before','after')]
                if (any(not isinstance(box,list) or len(box) != 2 or any(not isinstance(p,list) or len(p) != 3 or not all(finite(v) for v in p) for p in box) for box in bounds)
                        or any(abs(a-b)>1e-9 for pa,pb in zip(*bounds) for a,b in zip(pa,pb))):
                    raise ValueError('Move Normal edited its reference bounds')
        for key in ('before', 'after'):
            validate_snapshot(value[key], len(operation['sources']))
        if len(value['before']['objects']) != len(operation['sources']):
            raise ValueError('incomplete transform source observation')
        geometry_state = lambda state: dict(groups=state['groups'], objects=[dict((key,field) for key,field in obj.items() if key != 'selected') for obj in state['objects']])
        changed = geometry_state(value['after']) != geometry_state(value['before'])
        for key, required in [('last', operation['sel_last']),
                              ('undo', operation['undo_redo'] and changed),
                              ('redo', operation['undo_redo'] and changed)]:
            if required:
                validate_snapshot(value[key], len(operation['sources']))
            elif value[key] is not None:
                raise ValueError('unexpected transform history observation')
        terminal = []
        for event in value['events']:
            if (not isinstance(event, dict) or not isinstance(event.get('name'), str)
                    or event.get('result') not in ('Success', 'Cancel', 'Nothing', 'Failure', 'ExitRhino')
                    or not isinstance(event.get('selected'), list)):
                raise ValueError('invalid transform command event')
            if event['name'] == operation['command']:
                terminal.append(event)
        if (len(terminal) != 1 or terminal[0].get('objects') != value['after']
                or value['succeeded'] != (terminal[0]['result'] == 'Success')):
            raise ValueError('incomplete transform command terminal event')
    return observed


def validate_snapshot(snapshot, source_count):
    if (not isinstance(snapshot, dict) or set(snapshot) != {'objects', 'groups'}
            or not isinstance(snapshot['objects'], list) or not isinstance(snapshot['groups'], list)):
        raise ValueError('invalid transform document observation')
    objects, groups = snapshot['objects'], snapshot['groups']
    for obj in objects:
        if (not isinstance(obj, dict)
                or set(obj) != {'source', 'selected', 'point', 'name', 'color', 'color_source', 'current_layer', 'groups'}
                or (obj['source'] is not None and (type(obj['source']) is not int or not 0 <= obj['source'] < source_count))
                or any(type(obj[key]) is not bool for key in ('selected', 'current_layer'))
                or not isinstance(obj['name'], str) or not isinstance(obj['color_source'], str)
                or not isinstance(obj['point'], list) or len(obj['point']) != 3
                or any(type(number) not in (int, float) for number in obj['point'])
                or not isinstance(obj['color'], list) or len(obj['color']) != 3
                or any(type(number) is not int or not 0 <= number <= 255 for number in obj['color'])
                or not isinstance(obj['groups'], list)
                or any(type(index) is not int or not 0 <= index < len(groups) for index in obj['groups'])
                or len(set(obj['groups'])) != len(obj['groups'])):
            raise ValueError('invalid transform object observation')
    sources = [obj['source'] for obj in objects if obj['source'] is not None]
    if sorted(sources) != list(range(source_count)):
        raise ValueError('missing or duplicate transform source identity')
    for index, group in enumerate(groups):
        members = [row for row, obj in enumerate(objects) if index in obj['groups']]
        if (not isinstance(group, dict) or group != dict(members=members)
                or any(type(member) is not int for member in group['members'])):
            raise ValueError('inconsistent transform group observation')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request')
    parser.add_argument('--scheme', required=True)
    parser.add_argument('--timeout', type=float, default=300)
    args = parser.parse_args()
    print(json.dumps(capture(load_request(args.request), OracleClient(settings_scheme=args.scheme), args.timeout), indent=2, allow_nan=False))


if __name__ == '__main__':
    main()
