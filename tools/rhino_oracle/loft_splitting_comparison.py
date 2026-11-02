"""Compare Loft model states without native helper selection or command traces."""
import math
from .client import OracleProtocolError
from .loft_splitting_probe import validate


def _numbers(value, length):
    return (isinstance(value, list) and len(value) == length
            and all(type(x) in (int, float) and math.isfinite(x) for x in value))


def canonical(operation, value, native):
    validate(operation)
    if operation.get('inspection', False):
        raise OracleProtocolError('raw Loft inspection uses the diagnostic comparison')
    fields = {'after', 'undo', 'redo'}
    extra = {'history', 'events', 'crease_events'} if native else set()
    if not isinstance(value, dict) or set(value) != fields | extra:
        raise OracleProtocolError('invalid Loft state fields')
    if native:
        if (not isinstance(value['history'], str) or 'Unknown command:' in value['history']
                or not isinstance(value['events'], list) or not isinstance(value['crease_events'], list)):
            raise OracleProtocolError('invalid Loft command trace')
        for key, name in [('events', 'Loft'), ('crease_events', 'CreaseSplitting')]:
            events = [e for e in value[key] if isinstance(e, dict) and e.get('name') == name]
            if len(events) != 1 or events[0].get('result') != 'Success':
                raise OracleProtocolError('Loft command did not finish once successfully')
    result = {}
    for phase in ['after', 'undo', 'redo']:
        state = value[phase]
        if not isinstance(state, list):
            raise OracleProtocolError('invalid Loft model state')
        records = []; sources = set(); products = 0
        for row in state:
            if not isinstance(row, dict):
                raise OracleProtocolError('invalid Loft object')
            source = row.get('source')
            if source is None:
                expected = {'source', 'is_solid', 'face_count', 'edge_count', 'vertex_count', 'bounds', 'faces'}
                if (type(row.get('is_solid')) is not bool
                        or any(type(row.get(k)) is not int or row[k] <= 0
                               for k in ['face_count', 'edge_count', 'vertex_count'])):
                    raise OracleProtocolError('invalid Loft topology')
                bounds = row.get('bounds')
                if (not isinstance(bounds, list) or len(bounds) != 2
                        or not all(_numbers(p, 3) for p in bounds)
                        or any(a > b for a, b in zip(*bounds))):
                    raise OracleProtocolError('invalid Loft bounds')
                faces = row.get('faces')
                if not isinstance(faces, list) or len(faces) != row['face_count']:
                    raise OracleProtocolError('invalid Loft face count')
                charts = []
                for face in faces:
                    if (not isinstance(face, dict)
                            or set(face) != {'domain_u', 'domain_v', 'reversed', 'samples'}
                            or type(face['reversed']) is not bool
                            or any(not _numbers(face[k], 2) or face[k][0] >= face[k][1]
                                   for k in ['domain_u', 'domain_v'])
                            or not isinstance(face['samples'], list) or len(face['samples']) != 25
                            or not all(_numbers(p, 3) for p in face['samples'])):
                        raise OracleProtocolError('invalid Loft chart or surface samples')
                    charts.append((face['domain_v'], face['domain_u']))
                if charts != sorted(charts):
                    raise OracleProtocolError('Loft faces are not in chart order')
                products += 1
            else:
                expected = {'source', 'unchanged'}
                if (type(source) is not int or not 0 <= source < len(operation['curves'])
                        or source in sources or type(row.get('unchanged')) is not bool):
                    raise OracleProtocolError('invalid Loft source identity')
                sources.add(source)
            if (set(row) != expected | ({'selected'} if native else set())
                    or (native and type(row['selected']) is not bool)):
                raise OracleProtocolError('Loft object fields differ')
            records.append({k: row[k] for k in sorted(expected)})
        if sources != set(range(len(operation['curves']))) or products != int(phase != 'undo'):
            raise OracleProtocolError('Loft source retention or result count differs')
        result[phase] = records
    return result
