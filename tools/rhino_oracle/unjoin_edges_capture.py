"""Capture edge separation after independently exporting exact owned sources."""
import argparse
import json

from .client import OracleClient, _owned_artifact_request, _validate_response, load_request
from .unjoin_edges_probe import validate as validate_api
from .unjoin_edge_command_probe import validate as validate_command


def validate_request(request):
    if (type(request.get('protocol_version')) is not int or request['protocol_version'] != 1
            or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1
            or not isinstance(request.get('operations'), list) or not 1 <= len(request['operations']) <= 128):
        raise ValueError('edge separation capture requires protocol 1, one iteration and 1 to 128 cases')
    identifiers = set()
    for operation in request['operations']:
        if not isinstance(operation, dict): raise ValueError('invalid edge separation operation')
        name = operation.get('op')
        if name not in ('brep_unjoin_edges', 'unjoin_edge_command'):
            raise ValueError('edge separation needs a dedicated request')
        (validate_api if name == 'brep_unjoin_edges' else validate_command)(operation)
        if operation['id'] in identifiers: raise ValueError('duplicate edge separation capture id')
        identifiers.add(operation['id'])


def capture(request, client=None, timeout=300):
    client = client or OracleClient()
    with _owned_artifact_request(request) as prepared:
        validate_request(prepared)
        exports = []
        for index, operation in enumerate(prepared['operations']):
            sources = [operation['source']] if operation['op'] == 'brep_unjoin_edges' else [source['brep'] for source in operation['sources']]
            for part, source in enumerate(sources):
                exports.append(dict(op='brep_remove_holes', id='source-%d-%d' % (index, part), source=source, loops=[]))
        for offset in range(0, len(exports), 128):
            batch = exports[offset:offset+128]
            export_request = dict(protocol_version=1, iterations=1, operations=batch)
            if 'tolerance' in prepared: export_request['tolerance'] = prepared['tolerance']
            exported = client.run_viboceros(export_request, timeout)
            _validate_response(exported, 'viboceros')
            if [row['id'] for row in exported['results']] != [op['id'] for op in batch]:
                raise ValueError('incomplete independently exported edge separation sources')
        return client.run_rhino(prepared, timeout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request')
    parser.add_argument('--timeout', type=float, default=300)
    args = parser.parse_args()
    print(json.dumps(capture(load_request(args.request), timeout=args.timeout), indent=2, sort_keys=True, allow_nan=False))


if __name__ == '__main__': main()
