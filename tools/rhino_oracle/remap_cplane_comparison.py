"""Explicit RemapCPlane state comparison; raw capture diagnostics stay intact."""
import copy
import re
from collections.abc import Mapping

from .client import OracleProtocolError
from .remap_cplane_probe import validate

STATES = ('before', 'after', 'after_script', 'undo', 'redo')
COMMON = frozenset(STATES + ('success', 'undo_returned', 'redo_returned'))
NATIVE_DIAGNOSTICS = frozenset(('macro', 'history', 'events'))
GENERATED = re.compile(r'Group[0-9]+\Z')


def _keys(value, required, optional=()):
    if (not isinstance(value, Mapping) or not set(required) <= value.keys()
            or value.keys() - set(required) - set(optional)):
        raise OracleProtocolError('invalid RemapCPlane comparison schema')


def _events(events, success, after):
    if not isinstance(events, list):
        raise OracleProtocolError('RemapCPlane events must be an array')
    completed = []
    for event in events:
        _keys(event, ('name', 'result', 'selected'), ('objects',))
        if event['selected'] != []:
            raise OracleProtocolError('unexpected RemapCPlane diagnostic selection schema')
        if event['name'] == 'RemapCPlane':
            _keys(event, ('name', 'result', 'selected', 'objects'))
            completed.append(event)
        elif event['name'] != 'SelID' or event['result'] != 'Success' or 'objects' in event:
            raise OracleProtocolError('unexpected RemapCPlane child command event')
    if len(completed) != 1:
        raise OracleProtocolError('RemapCPlane needs exactly one EndCommand observation')
    event = completed[0]
    if ((event['result'] == 'Success') != success or event['objects'] != after
            or event['result'] not in ('Success', 'Cancel', 'Failure', 'Nothing')):
        raise OracleProtocolError('RemapCPlane EndCommand contradicts the retained outcome')


def _normalize_state(state, generated_names):
    _keys(state, ('objects', 'groups', 'plane'))
    if not isinstance(state['objects'], list) or not isinstance(state['groups'], list):
        raise OracleProtocolError('RemapCPlane state needs objects and groups arrays')
    result = copy.deepcopy(state)
    seen = set()
    for group in result['groups']:
        _keys(group, ('name', 'count'))
        name = group['name']
        if not isinstance(name, str) or name in seen or type(group['count']) is not int or group['count'] < 0:
            raise OracleProtocolError('invalid RemapCPlane group observation')
        seen.add(name)
        if name != 'Owned remap group':
            if not GENERATED.fullmatch(name):
                raise OracleProtocolError('unexpected RemapCPlane generated group name')
            # Keep identities consistent across history phases. A rename or
            # replacement during Undo/Redo remains a detectable difference.
            group['name'] = generated_names.setdefault(name, 'generated/' + str(len(generated_names)))
    return result


def canonical(operation, value, *, native):
    """Compare all exported state; only driver diagnostics are nonportable."""
    try:
        validate(operation)
    except (TypeError, ValueError) as error:
        raise OracleProtocolError(str(error)) from error
    required = COMMON | (NATIVE_DIAGNOSTICS if native else {'target_rejections'})
    optional = {'setup'} if 'setup_copy' in operation else set()
    _keys(value, required, optional)
    if any(type(value[key]) is not bool for key in ('success', 'undo_returned', 'redo_returned')):
        raise OracleProtocolError('RemapCPlane outcome flags must be booleans')
    if ('setup' in value) != ('setup_copy' in operation):
        raise OracleProtocolError('RemapCPlane setup observation does not match the request')
    if native:
        if not isinstance(value['macro'], str) or not isinstance(value['history'], str):
            raise OracleProtocolError('RemapCPlane diagnostics must be strings')
        _events(value['events'], value['success'], value['after'])
        rejections = value['history'].count('Specify a different CPlane.')
    else:
        rejections = value['target_rejections']
        if type(rejections) is not int or rejections < 0:
            raise OracleProtocolError('RemapCPlane target rejection count must be a nonnegative integer')
    names = {}
    result = {key: value[key] for key in ('success', 'undo_returned', 'redo_returned')}
    result['target_rejections'] = rejections
    if 'setup' in value:
        setup = value['setup']
        _keys(setup, ('success', 'after') + (('events', 'macro') if native else ()))
        if type(setup['success']) is not bool:
            raise OracleProtocolError('RemapCPlane setup outcome must be boolean')
        if native:
            if not isinstance(setup['macro'], str):
                raise OracleProtocolError('RemapCPlane setup macro must be a string')
            _events(setup['events'], setup['success'], setup['after'])
        result['setup'] = {'success': setup['success'], 'after': _normalize_state(setup['after'], names)}
    for key in STATES:
        result[key] = _normalize_state(value[key], names)
    return result
