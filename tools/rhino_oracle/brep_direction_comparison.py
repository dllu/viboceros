"""Retain every Dir model-state field while validating native-only traces."""
import math
from .client import OracleProtocolError
from .brep_direction_probe import validate


def canonical(operation, value, native):
    validate(operation)
    phases={'before','after','undo','redo'}
    extra={'success','events','history','macro','sdk'} if native else set()
    if not isinstance(value,dict) or set(value)!=phases|extra:
        raise OracleProtocolError('invalid Dir state fields')
    if native:
        def geometries(phase):
            rows=value.get(phase)
            if operation.get('pair',False):
                return [row.get('geometry') for row in rows] if isinstance(rows,list) and all(isinstance(row,dict) for row in rows) else None
            return rows.get('geometry') if isinstance(rows,dict) else None
        changed=geometries('before') != geometries('after')
        success=(not operation.get('cancel',False)) if operation.get('pair',False) else (changed if 'sequence' in operation else True)
        if (value['success'] is not success or value['sdk'] is not operation.get('sdk',False)
                or not isinstance(value['events'],list) or not isinstance(value['history'],str)
                or not isinstance(value['macro'],str) or 'Unknown command:' in value['history']):
            raise OracleProtocolError('Dir trace did not succeed')
        events=[e for e in value['events'] if isinstance(e,dict) and e.get('name')=='Dir']
        if operation.get('sdk',False):
            if value['events']:raise OracleProtocolError('SDK replacement cannot claim a Dir event')
        elif len(events)!=1 or events[0].get('result')!=('Success' if success else ('Cancel' if operation.get('pair',False) else 'Nothing')):
            raise OracleProtocolError('Dir command must finish once')
    def finite(tree):
        if isinstance(tree,dict):return all(isinstance(k,str) and finite(v) for k,v in tree.items())
        if isinstance(tree,list):return all(finite(v) for v in tree)
        if type(tree) is float:return math.isfinite(tree)
        return tree is None or type(tree) in (str,int,bool)
    def check_row(row):
        if (not isinstance(row,dict) or set(row)!={'geometry','selected','name','groups'}
                or type(row['selected']) is not bool or not isinstance(row['name'],str)
                or type(row['groups']) is not int or row['groups']<0 or not finite(row)):
            raise OracleProtocolError('invalid Dir object state')
        g=row['geometry']
        if (not isinstance(g,dict) or set(g)!={'topology','vertices','edges','faces'}
                or any(not isinstance(g[k],list) for k in ('vertices','edges','faces'))
                or not g['faces'] or not isinstance(g['topology'],dict)):
            raise OracleProtocolError('invalid Dir B-rep record')
        topology=g['topology']
        if (type(topology.get('vertices')) is not int or topology['vertices']!=len(g['vertices'])
                or not isinstance(topology.get('edges'),list) or len(topology['edges'])!=len(g['edges'])
                or not isinstance(topology.get('faces'),list) or len(topology['faces'])!=len(g['faces'])
                or type(topology.get('solid')) is not bool):
            raise OracleProtocolError('Dir topology counts differ')
    for phase in phases:
        if operation.get('pair',False):
            rows=value[phase]
            if not isinstance(rows,list) or len(rows)!=2:raise OracleProtocolError('invalid paired Dir state')
            for row in rows:check_row(row)
        else:check_row(value[phase])
    return {phase:value[phase] for phase in sorted(phases)}
