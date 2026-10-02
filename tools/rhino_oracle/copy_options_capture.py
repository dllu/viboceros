"""Capture shared Copy settings after exporting an independent input B-rep."""
import argparse
import json

from .client import OracleClient, _owned_artifact_request, _validate_response, load_request
from .copy_options_probe import validate


def validate_request(request):
    if (not isinstance(request, dict)
            or type(request.get('protocol_version')) is not int or request['protocol_version'] != 1
            or type(request.get('iterations', 1)) is not int or request.get('iterations', 1) != 1
            or not isinstance(request.get('operations'), list) or len(request['operations']) != 1
            or not isinstance(request['operations'][0], dict)):
        raise ValueError('Copy settings capture requires protocol 1, one workflow, and one iteration')


def capture(request, client, timeout=300):
    if client.settings_scheme is None:
        raise ValueError('Copy settings capture requires a private scheme')
    validate_request(request)
    with _owned_artifact_request(request) as prepared:
        operation = prepared['operations'][0]
        validate(operation)
        export = dict(protocol_version=1, iterations=1, operations=[dict(
            op='brep_remove_holes', id='source', source=operation['sources'][0]['brep'], loops=[])])
        if 'tolerance' in prepared:
            export['tolerance'] = prepared['tolerance']
        response = client.run_viboceros(export, timeout)
        _validate_response(response, 'viboceros')
        if response['iterations'] != 1 or [row['id'] for row in response['results']] != ['source']:
            raise ValueError('incomplete Copy workflow export')
        observed = client.run_rhino(prepared, timeout)
        _validate_response(observed, 'rhino')
        if observed['iterations'] != 1 or [row['id'] for row in observed['results']] != [operation['id']]:
            raise ValueError('incomplete Copy workflow capture')
        records = observed['results'][0]['value'].get('records')
        if not isinstance(records, list) or len(records) != len(operation['steps']):
            raise ValueError('incomplete Copy workflow steps')
        for step, record in zip(operation['steps'], records):
            if (not isinstance(record, dict) or any(record.get(key) != value for key, value in step.items())
                    or type(record.get('succeeded')) is not bool
                    or not isinstance(record.get('before'), list) or not isinstance(record.get('after'), list)
                    or not isinstance(record.get('events'), list)):
                raise ValueError('invalid Copy workflow observation')
        return observed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request')
    parser.add_argument('--scheme', required=True)
    parser.add_argument('--timeout', type=float, default=300)
    args = parser.parse_args()
    observed = capture(load_request(args.request), OracleClient(settings_scheme=args.scheme), args.timeout)
    print(json.dumps(observed, indent=2, allow_nan=False))


if __name__ == '__main__':
    main()
