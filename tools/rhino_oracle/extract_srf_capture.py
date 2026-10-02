"""Capture owned ExtractSrf only after exporting independent source fixtures."""
import argparse
import json
from .client import OracleClient, _owned_artifact_request, _validate_response, load_request
from .extract_srf_probe import validate


def validate_request(request):
    if (type(request.get('protocol_version')) is not int or request['protocol_version']!=1
            or type(request.get('iterations',1)) is not int or request.get('iterations',1)!=1
            or not isinstance(request.get('operations'),list) or not 1<=len(request['operations'])<=128):
        raise ValueError('ExtractSrf capture requires protocol 1, one iteration, and 1 to 128 cases')
    seen=set()
    for operation in request['operations']:
        validate(operation)
        if operation['id'] in seen:raise ValueError('duplicate ExtractSrf capture id')
        seen.add(operation['id'])


def capture(request,client=None,timeout=300):
    client=client or OracleClient()
    with _owned_artifact_request(request) as prepared:
        validate_request(prepared);exports=[]
        for index,operation in enumerate(prepared['operations']):
            for part,source in enumerate(operation['sources']):
                exports.append(dict(op='brep_remove_holes',id='source-%d-%d'%(index,part),source=source['brep'],loops=[]))
        for offset in range(0,len(exports),128):
            batch=exports[offset:offset+128]
            export_request=dict(protocol_version=1,iterations=1,operations=batch)
            if 'tolerance' in prepared:export_request['tolerance']=prepared['tolerance']
            exported=client.run_viboceros(export_request,timeout);_validate_response(exported,'viboceros')
            if [row['id'] for row in exported['results']]!=[op['id'] for op in batch]:
                raise ValueError('incomplete independent ExtractSrf source exports')
        combined=None
        for offset in range(0,len(prepared['operations']),8):
            native=client.run_rhino(dict(prepared,operations=prepared['operations'][offset:offset+8]),timeout)
            _validate_response(native,'rhino')
            if [row['id'] for row in native['results']]!=[op['id'] for op in prepared['operations'][offset:offset+8]]:
                raise ValueError('incomplete owned ExtractSrf observations')
            if combined is None:combined=dict(native,results=[])
            if any(native.get(key)!=combined.get(key) for key in ('protocol_version','engine','engine_version','iterations')):
                raise ValueError('ExtractSrf capture engines changed between batches')
            combined['results'].extend(native['results'])
        return combined


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('request');parser.add_argument('--timeout',type=float,default=300)
    args=parser.parse_args();print(json.dumps(capture(load_request(args.request),timeout=args.timeout),indent=2,allow_nan=False))


if __name__=='__main__':main()
