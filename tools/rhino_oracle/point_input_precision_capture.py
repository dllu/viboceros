"""Bounded owned-session batches for the public point-input precision matrix."""
import argparse
import hashlib
import json
from pathlib import Path

from .client import OracleClient, OracleProtocolError, _validate_response
from .point_input_precision_probe import validate_request


def source_hashes():
    root = Path(__file__).parent
    names = ('client.py','rhino_worker.py','point_input_precision_capture.py',
             'point_input_precision_probe.py','number_token.py','join_probe.py','merge_edges_probe.py',
             'run_headless.sh','i3-headless.conf')
    return {name:hashlib.sha256((root/name).read_bytes()).hexdigest() for name in names}


def write_json(path,value):
    temporary = path.with_name(path.name+'.tmp')
    temporary.write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')
    temporary.replace(path)


def capture(request,scheme,timeout=240,batch_size=16,checkpoint_dir=None):
    validate_request(request)
    if type(batch_size) is not int or not 1 <= batch_size <= 16:
        raise ValueError('point precision batch size must be from 1 through 16')
    results,batches,header = [],[],None
    hashes = source_hashes()
    if checkpoint_dir is not None:
        checkpoint_dir = Path(checkpoint_dir)
        checkpoint_dir.mkdir(parents=True,exist_ok=True)
    operations = request['operations']
    for offset in range(0,len(operations),batch_size):
        subset = dict(request,operations=operations[offset:offset+batch_size])
        owned_scheme = scheme+'_%02d' % (offset//batch_size)
        print('capturing %d..%d in %s' % (offset,offset+len(subset['operations'])-1,owned_scheme),flush=True)
        checkpoint = checkpoint_dir/('%03d.json' % offset) if checkpoint_dir is not None else None
        identity = dict(request=subset,settings_scheme=owned_scheme,source_sha256=hashes)
        if checkpoint is not None and checkpoint.exists():
            saved = json.loads(checkpoint.read_text())
            if {key:saved.get(key) for key in identity}!=identity:
                raise OracleProtocolError('point precision checkpoint inputs or sources differ')
            response = saved['response']
        else:
            response = OracleClient(settings_scheme=owned_scheme).run_rhino(subset,timeout)
        _validate_response(response,'rhino')
        metadata = {key:response[key] for key in ('engine','engine_version','protocol_version','iterations')}
        if header is None: header = metadata
        if metadata!=header: raise OracleProtocolError('point precision batch engines differ')
        rows = response.get('results',[])
        if [row.get('id') for row in rows]!=[op['id'] for op in subset['operations']]:
            raise OracleProtocolError('point precision batch output differs from requested order')
        if checkpoint is not None:
            write_json(checkpoint,dict(identity,response=response))
        results.extend(rows)
        batches.append(dict(settings_scheme=owned_scheme,ids=[op['id'] for op in subset['operations']]))
        print('captured %d/%d' % (len(results),len(operations)),flush=True)
    return dict(header,results=results,capture_batches=batches)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request',type=Path)
    parser.add_argument('--scheme',required=True)
    parser.add_argument('--output',required=True,type=Path)
    parser.add_argument('--timeout',type=float,default=240)
    parser.add_argument('--batch-size',type=int,default=16)
    parser.add_argument('--checkpoints',type=Path,
                        help='retain completed batches; reuse only with identical inputs and sources')
    args = parser.parse_args()
    checkpoints = args.checkpoints or args.output.with_name(args.output.name+'.batches')
    response = capture(json.loads(args.request.read_text()),args.scheme,args.timeout,args.batch_size,checkpoints)
    write_json(args.output,response)


if __name__=='__main__': main()
