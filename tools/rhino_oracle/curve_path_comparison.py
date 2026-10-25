"""Strict model-state comparison; keep native helper selection in the archive."""
import math
from .client import OracleProtocolError
from .curve_path_extrusion_probe import validate


def canonical(operation, value, native):
    validate(operation)
    keys={'succeeded','after','undo','redo'}
    extra={'history','events','crease_splitting','crease_events'} if native else set()
    if not isinstance(value,dict) or set(value) != keys|extra or type(value['succeeded']) is not bool:
        raise OracleProtocolError('invalid curve path extrusion result')
    if native:
        if not isinstance(value['history'],str) or not isinstance(value['events'],list) or not isinstance(value['crease_events'],list):
            raise OracleProtocolError('invalid curve path extrusion command trace')
        events=[e for e in value['events'] if isinstance(e,dict) and e.get('name')=='ExtrudeCrvAlongCrv']
        creases=[e for e in value['crease_events'] if isinstance(e,dict) and e.get('name')=='CreaseSplitting']
        if (len(events)!=1 or events[0].get('result')!=('Success' if value['succeeded'] else 'Failure')
                or len(creases)!=1 or creases[0].get('result')!='Success'
                or type(value['crease_splitting']) is not bool or value['crease_splitting']!=operation['crease_splitting']):
            raise OracleProtocolError('curve path native command did not finish as recorded')
    result={'succeeded':value['succeeded']}
    for phase in ['after','undo','redo']:
        state=value[phase]
        if not value['succeeded'] and phase!='after':
            if state is not None:raise OracleProtocolError('failed curve path command has an undo replay')
            result[phase]=None
            continue
        if not isinstance(state,list):raise OracleProtocolError('invalid curve path model state')
        records=[];sources=[];products=0
        for row in state:
            if not isinstance(row,dict):raise OracleProtocolError('invalid curve path object')
            source=row.get('source')
            if source is None:
                expected={'source','is_solid','face_count','volume','bounds'}
                if type(row.get('is_solid')) is not bool or type(row.get('face_count')) is not int or row['face_count']<=0:
                    raise OracleProtocolError('invalid curve path product topology')
                bounds=row.get('bounds')
                if (not isinstance(bounds,list) or len(bounds)!=2
                        or any(not isinstance(p,list) or len(p)!=3 for p in bounds)
                        or any(type(x) not in (int,float) or not math.isfinite(x) for p in bounds for x in p)
                        or any(a>b for a,b in zip(bounds[0],bounds[1]))):
                    raise OracleProtocolError('invalid curve path bounds')
                volume=row.get('volume')
                if (row['is_solid'] and (type(volume) not in (int,float) or not math.isfinite(volume) or volume<0)) or (not row['is_solid'] and volume is not None):
                    raise OracleProtocolError('invalid curve path volume')
                products+=1
            else:
                expected={'source','unchanged'}
                if type(source) is not int or source not in [0,1] or source in sources or type(row.get('unchanged')) is not bool:
                    raise OracleProtocolError('invalid curve path source identity')
                sources.append(source)
            if set(row)!=expected|({'selected'} if native else set()) or (native and type(row['selected']) is not bool):
                raise OracleProtocolError('curve path object fields differ')
            records.append({k:row[k] for k in sorted(expected)})
        if 1 not in sources or products != int(value['succeeded'] and phase!='undo'):
            raise OracleProtocolError('curve path source or product count differs')
        result[phase]=records
    return result
