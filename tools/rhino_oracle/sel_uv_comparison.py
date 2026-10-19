"""Compare complete portable SelUV states; retain native driver diagnostics."""


def canonical(operation, value, *, native):
    from .client import OracleProtocolError
    flags = {'selected','visible','grips_on','peer_selected','peer_visible'}
    state_keys = flags | {'grips'}
    def state(value):
        if (not isinstance(value, dict) or set(value) != state_keys
                or any(type(value[k]) is not bool for k in flags)
                or not isinstance(value['grips'], list)):
            raise OracleProtocolError('invalid SelUV state')
        for i, g in enumerate(value['grips']):
            if (not isinstance(g,dict) or set(g) != {'index','point','selected','hidden'}
                    or type(g['index']) is not int or g['index'] != i
                    or type(g['selected']) is not bool or type(g['hidden']) is not bool
                    or not isinstance(g['point'],list) or len(g['point']) != 3):
                raise OracleProtocolError('invalid SelUV grip identity or flags')
        return value
    def command(value, cancelled):
        out = {k:state(value[k]) for k in ('before','after','after_script')}
        if native:
            events = [e for e in value['events'] if e['name'] == 'SelUV']
            if (len(events) != 1 or events[0]['result'] != ('Cancel' if cancelled else 'Success')
                    or events[0]['objects'] != value['after'] or value['success'] != (not cancelled)):
                raise OracleProtocolError('SelUV command did not complete as recorded')
        return out
    keys = {'before','after','after_script','redo_retained'} | ({'again'} if 'next_actions' in operation else set())
    if not native and set(value) != keys:
        raise OracleProtocolError('unexpected SelUV replay state fields')
    out = command(value,operation.get('cancel',False) or operation.get('cancel_surface',False))
    if type(value['redo_retained']) is not bool:
        raise OracleProtocolError('invalid SelUV Redo witness')
    out['redo_retained'] = value['redo_retained']
    if 'next_actions' in operation: out['again'] = command(value['again'],False)
    return out
