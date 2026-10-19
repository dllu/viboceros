"""Managed UI-thread input records SelUV state while its public getter runs."""


def drive(host, source, peer, tokens, count):
    if tokens and tokens[0] == '_SelID':
        tokens = [' '.join(tokens[:2])] + tokens[2:]
    import clr
    R, S = host['Rhino'], host['System']
    clr.AddReference('System.Core')
    from System.Linq.Expressions import Expression as E, ParameterExpression
    state = S.Array[S.Int32]([0, 1, 0])
    stride = 4 + 2 * count
    flags = S.Array[S.Boolean]([False] * (stride * len(tokens)))
    prompts = S.Array[S.String]([None] * len(tokens))
    last = S.Array[S.DateTime]([S.DateTime.UtcNow])
    started = S.DateTime.UtcNow
    array = lambda values: S.Array[E](values)
    integer = lambda value: E.Constant(S.Int32(value))
    item = lambda values, index: E.ArrayAccess(E.Constant(values), array([integer(index)]))
    block = lambda statements: E.Block(array(statements + [E.Empty()]))
    def method(kind, name, types):
        return clr.GetClrType(kind).GetMethod(name, S.Array[S.Type]([clr.GetClrType(t) for t in types]))
    def call(target, member, arguments): return E.Call(target, member, array(arguments))
    def compile(kind, body, parameters=()):
        return E.Lambda(clr.GetClrType(kind), body, S.Array[ParameterExpression](parameters)).Compile()
    obj = E.Constant(source, clr.GetClrType(R.DocObjects.RhinoObject))
    other = E.Constant(peer, clr.GetClrType(R.DocObjects.RhinoObject))
    now = E.Property(None, clr.GetClrType(S.DateTime).GetProperty('UtcNow'))
    prompt = E.Property(None, clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))
    elapsed = lambda start: E.Property(call(now, method(S.DateTime, 'Subtract', [S.DateTime]), [start]), 'TotalSeconds')
    send = method(R.RhinoApp, 'SendKeystrokes', [S.String, S.Boolean])
    get_grips = method(R.DocObjects.RhinoObject, 'GetGrips', [])
    selected = method(R.DocObjects.RhinoObject, 'IsSelected', [S.Boolean])
    active = E.Equal(item(state, 1), integer(1))
    ready = E.AndAlso(E.GreaterThan(elapsed(item(last, 0)), E.Constant(.3)),
        call(prompt, method(S.String, 'StartsWith', [S.String]), [E.Constant('Select ')]))
    branches = []
    for index, token in enumerate(tokens):
        offset = index * stride
        capture = [E.Assign(item(prompts, index), prompt),
            E.Assign(item(flags, offset), E.Property(obj, 'GripsOn')),
            E.Assign(item(flags, offset + 1), E.Property(obj, 'IsHidden')),
            E.Assign(item(flags, offset + 2), E.Property(other, 'IsHidden')),
            E.Assign(item(flags, offset + 3), E.NotEqual(call(obj, selected, [E.Constant(False)]), integer(0)))]
        grips = call(obj, get_grips, [])
        values = []
        for grip in range(count):
            g = E.ArrayIndex(grips, integer(grip))
            values += [E.Assign(item(flags, offset + 4 + grip * 2), E.Property(g, 'IsHidden')),
                E.Assign(item(flags, offset + 5 + grip * 2), E.NotEqual(call(g, selected, [E.Constant(False)]), integer(0)))]
        capture += [E.IfThen(E.Property(obj, 'GripsOn'), block(values)),
            E.Assign(item(state, 0), integer(index + 1)), E.Assign(item(last, 0), now),
            call(None, send, [E.Constant(token), E.Constant(True)])]
        branches.append(E.IfThen(E.Equal(item(state, 0), integer(index)), block(capture)))
    # A branch may reenter the UI loop while sending input. Stop this tick after
    # one token instead of advancing through newly satisfied branches.
    done = E.Label(); body = []
    for index, branch in enumerate(branches):
        body.append(E.IfThen(E.Equal(item(state, 0), integer(index)), block([branch, E.Goto(done)])))
    body.append(E.Label(done))
    action = compile(S.Action, E.IfThen(E.AndAlso(active, ready), block(body)))
    cancel = compile(S.Action, E.IfThen(E.AndAlso(active, E.GreaterThan(elapsed(E.Constant(started)), E.Constant(30.))),
        block([E.Assign(item(state, 2), integer(1)), call(None, send, [E.Constant('_Cancel'), E.Constant(True)])])))
    invoke = method(R.RhinoApp, 'InvokeOnUiThread', [S.Delegate, S.Array[S.Object]])
    def callback(action):
        arg = E.Parameter(clr.GetClrType(S.Object), 'state')
        return compile(S.Threading.TimerCallback, block([call(None, invoke, [
            E.Constant(action, clr.GetClrType(S.Delegate)), E.Constant(S.Array[S.Object]([]))])]), [arg])
    timer = S.Threading.Timer(callback(action), None, 100, 100)
    timeout = S.Threading.Timer(callback(cancel), None, 1000, 500)
    try:
        returned = R.RhinoApp.RunScript('_SelUV _Pause', False)
    finally:
        state[1] = 0; timer.Dispose(); timeout.Dispose()
    if state[2] or state[0] != len(tokens): raise ValueError('incomplete managed SelUV getter input')
    trace = []
    for index, token in enumerate(tokens):
        offset = index * stride
        trace.append(dict(token=token, prompt=prompts[index], grips_on=bool(flags[offset]),
            visible=not bool(flags[offset + 1]), peer_visible=not bool(flags[offset + 2]),
            selected=bool(flags[offset + 3]),
            hidden=[i for i in range(count) if flags[offset + 4 + i * 2]],
            picks=[i for i in range(count) if flags[offset + 5 + i * 2]]))
    return returned, trace
