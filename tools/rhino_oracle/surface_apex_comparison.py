"""Compare portable command states; retain native command transcripts separately."""
from .client import OracleProtocolError
from .surface_apex_command_probe import validate


def canonical(operation, value, native):
    validate(operation)
    if operation.get('surface_details', False):
        raise OracleProtocolError('surface definitions are native inspection diagnostics')
    keys = {'after','undo','redo','document_tolerance'}
    if not isinstance(value, dict) or set(value) != keys | ({'history','events'} if native else set()):
        raise OracleProtocolError('surface apex command result fields differ')
    if native:
        if not isinstance(value['history'],str) or not isinstance(value['events'],list):
            raise OracleProtocolError('surface apex native transcript is invalid')
        events = [e for e in value['events'] if isinstance(e,dict) and e.get('name') == 'ExtrudeSrfToPoint']
        if len(events) != 1 or events[0].get('result') != 'Success':
            raise OracleProtocolError('surface apex native command did not succeed exactly once')
    for phase in ['after','undo','redo']:
        state = value[phase]
        if (not isinstance(state,dict) or set(state) != {'source_present','source_unchanged','breps'}
                or type(state['source_present']) is not bool or type(state['source_unchanged']) is not bool
                or not isinstance(state['breps'],list)):
            raise OracleProtocolError('surface apex command state is invalid')
        for brep in state['breps']:
            if (not isinstance(brep,dict) or set(brep) != {'face_count','is_solid','volume','bounds'}
                    or type(brep['face_count']) is not int or brep['face_count'] <= 0
                    or type(brep['is_solid']) is not bool):
                raise OracleProtocolError('surface apex B-rep diagnostics are invalid')
    if not isinstance(value['document_tolerance'],dict) or set(value['document_tolerance']) != {'absolute','relative','angular'}:
        raise OracleProtocolError('surface apex document tolerances are invalid')
    return {key:value[key] for key in keys}
