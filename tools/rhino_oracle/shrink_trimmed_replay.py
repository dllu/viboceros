"""Replay independent shrink sources, preserving every numeric topology index.

Native object renewal order can vary between identical owned inputs. Compare
snapshots by source identity; keep the raw observed order in capture artifacts.
"""
import argparse
import copy
import json
from .client import OracleClient, OracleProtocolError, compare_responses, load_request, _owned_artifact_request
from .shrink_trimmed_probe import validate


def canonical_response(request,response):
    operations=request['operations']
    if (response.get('protocol_version')!=1 or response.get('iterations')!=1
            or not isinstance(response.get('results'),list) or len(response['results'])!=len(operations)):
        raise OracleProtocolError('incomplete shrink response')
    result=copy.deepcopy(response)
    for op,row in zip(operations,result['results']):
        if row.get('id')!=op['id']:raise OracleProtocolError('shrink operation order mismatch')
        value=row['value']
        for field in ('events','history','undo_events','redo_events'):value.pop(field,None)
        for field in ('before','after','undo','redo'):
            if field not in value:continue
            objects=value[field]
            if (not isinstance(objects,list) or any(type(obj.get('source')) is not int for obj in objects)
                    or sorted(obj['source'] for obj in objects)!=list(range(len(op['sources'])))):
                raise OracleProtocolError('shrink snapshot lost or duplicated source identities')
            objects.sort(key=lambda obj:obj['source'])
    return result


def replay(request,observed,client=None,timeout=180):
    if (request.get('protocol_version')!=1 or type(request.get('iterations')) is not int
            or request['iterations']!=1 or not request.get('operations') or observed.get('engine')!='rhino'):
        raise OracleProtocolError('expected single-iteration shrink inputs and Rhino capture')
    with _owned_artifact_request(request) as prepared:
        for operation in prepared['operations']:validate(operation)
        expected=canonical_response(request,observed)
        actual=(client or OracleClient()).run_viboceros(prepared,timeout)
    return compare_responses(canonical_response(request,actual),expected,1e-9,0.)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request');parser.add_argument('observations');parser.add_argument('--timeout',type=float,default=180)
    args=parser.parse_args();report=replay(load_request(args.request),load_request(args.observations),timeout=args.timeout)
    print(json.dumps(report.as_dict(),indent=2,allow_nan=False))
    return 0 if report.passed else 1


if __name__=='__main__':raise SystemExit(main())
