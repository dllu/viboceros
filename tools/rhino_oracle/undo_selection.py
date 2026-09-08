"""Validation for isolated whole-object undo-selection probes."""
from .client import OracleProtocolError

def validate_request(request):
    if type(request.get('protocol_version')) is not int or request['protocol_version'] != 1 or type(request.get('iterations',1)) is not int or request.get('iterations',1) != 1:
        raise OracleProtocolError('undo selection requires protocol 1 and one iteration')
    operations=request.get('operations')
    if not isinstance(operations,list) or not 1 <= len(operations) <= 64:
        raise OracleProtocolError('undo selection requires 1 to 64 cases')
    ids=set()
    for op in operations:
        if not isinstance(op,dict) or op.get('op') != 'undo_selection' or op.get('kind') not in ('Move','Delete','Explode','DeleteFaces','ExtractMeshFaces') or type(op.get('clear')) is not bool:
            raise OracleProtocolError('invalid undo selection case')
        key=op.get('id')
        if not isinstance(key,str) or not key.strip() or len(key)>100 or not key.isascii() or not all(c.isalnum() or c in '_-' for c in key) or key in ids:
            raise OracleProtocolError('invalid undo selection id')
        ids.add(key)
