"""Compare complete saved API and command captures without a live Rhino GUI."""
import argparse
import copy
import json

from .client import OracleClient, OracleProtocolError, _owned_artifact_request, compare_responses, load_request
from .unjoin_edges_capture import validate_request


def canonical_response(request, response):
    if (response.get('protocol_version') != 1 or response.get('iterations') != 1
            or len(response.get('results', [])) != len(request['operations'])):
        raise OracleProtocolError('incomplete edge separation response')
    result = copy.deepcopy(response)
    for operation, row in zip(request['operations'], result['results']):
        if row.get('id') != operation['id']: raise OracleProtocolError('edge separation order mismatch')
        if operation['op'] == 'unjoin_edge_command':
            for field in ('events', 'history', 'undo_events', 'redo_events'):
                row['value'].pop(field, None)
    return result


def replay(request, observed, client=None, timeout=300):
    if observed.get('engine') != 'rhino': raise OracleProtocolError('expected saved Rhino capture')
    with _owned_artifact_request(request) as prepared:
        validate_request(prepared)
        expected = canonical_response(request, observed)
        actual = (client or OracleClient()).run_viboceros(prepared, timeout)
    return compare_responses(canonical_response(request, actual), expected, 1e-9, 0.)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request')
    parser.add_argument('observations')
    parser.add_argument('--timeout', type=float, default=300)
    args = parser.parse_args()
    report = replay(load_request(args.request), load_request(args.observations), timeout=args.timeout)
    print(json.dumps(report.as_dict(), indent=2, allow_nan=False))
    return 0 if report.passed else 1


if __name__ == '__main__': raise SystemExit(main())
