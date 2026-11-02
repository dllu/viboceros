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
        if (value['success'] is not True or value['sdk'] is not operation.get('sdk',False)
                or not isinstance(value['events'],list) or not isinstance(value['history'],str)
                or not isinstance(value['macro'],str) or 'Unknown command:' in value['history']):
            raise OracleProtocolError('Dir trace did not succeed')
        events=[e for e in value['events'] if isinstance(e,dict) and e.get('name')=='Dir']
        if operation.get('sdk',False):
            if value['events']:raise OracleProtocolError('SDK replacement cannot claim a Dir event')
        elif len(events)!=1 or events[0].get('result')!='Success':
            raise OracleProtocolError('Dir command must finish once')
    def finite(tree):
        if isinstance(tree,dict):return all(isinstance(k,str) and finite(v) for k,v in tree.items())
        if isinstance(tree,list):return all(finite(v) for v in tree)
        if type(tree) is float:return math.isfinite(tree)
        return tree is None or type(tree) in (str,int,bool)
    for phase in phases:
        row=value[phase]
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
    return {phase:value[phase] for phase in sorted(phases)}
