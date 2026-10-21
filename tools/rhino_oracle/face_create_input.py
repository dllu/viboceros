"""Bounded physical edge/Escape input in an owned 3DFace getter."""
import os


def drive(op, case, command, host):
    R, S = host['Rhino'], host['System']
    doc = R.RhinoDoc.ActiveDoc
    import clr
    clr.AddReference('System.Core')
    from System.Linq.Expressions import Expression as E, ParameterExpression
    state = S.Array[S.Int32]([0, 1])
    observed = S.Array[S.String]([None] * 4)
    progress = os.path.join(os.path.dirname(host['__file__']), 'worker-progress.log')
    def array(v): return S.Array[E](v)
    def integer(v): return E.Constant(S.Int32(v))
    def item(v, i): return E.ArrayAccess(E.Constant(v), array([integer(i)]))
    def method(t, name, types): return clr.GetClrType(t).GetMethod(name, S.Array[S.Type]([clr.GetClrType(v) for v in types]))
    def call(target, m, args): return E.Call(target, m, array(args))
    def block(v): return E.Block(array(v + [E.Empty()]))
    def compile_delegate(t, body, parameters=()): return E.Lambda(clr.GetClrType(t), body, S.Array[ParameterExpression](parameters)).Compile()
    prompt = E.Property(None, clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))
    starts = method(S.String, 'StartsWith', [S.String])
    append = method(S.IO.File, 'AppendAllText', [S.String, S.String])
    stage = item(state, 0)
    markers = []
    if case.get('edge_pick'):
        view = doc.Views.ActiveView
        vp = view.ActiveViewport
        vp.SetProjection(R.Display.DefinedViewportProjection.Top, 'Owned 3DFace edge', False)
        vp.ZoomExtents(); view.Redraw()
        p = vp.WorldToClient(R.Geometry.Point3d(*case['edge_pick']))
        screen = view.ClientToScreen(S.Drawing.Point(int(p.X), int(p.Y)))
        markers = ['\nPICK %s %d %d\n' % (op['id'], screen.X, screen.Y)]
        gate = call(prompt, starts, [E.Constant('Pick point')])
    else:
        markers = ['\nPICK_ABORT %s-%d\n' % (op['id'], i) for i in range(4)]
        in_point = method(R.Input.RhinoGet, 'InGetPoint', [R.RhinoDoc])
        gate = E.AndAlso(call(prompt, starts, [E.Constant('Pick point')]), call(None, in_point, [E.Constant(doc, clr.GetClrType(R.RhinoDoc))]))
    branches = []
    for i, marker in enumerate(markers):
        branches.append(E.IfThen(E.Equal(stage, integer(i)), block([
            E.Assign(item(observed, i), prompt), E.Assign(stage, integer(i + 1)),
            call(None, append, [E.Constant(progress), E.Constant(marker)])])))
    # One action per timer tick, even when the next prompt has identical text.
    body = branches[-1]
    for branch, i in reversed(list(zip(branches[:-1], range(len(branches)-1)))):
        body = E.IfThenElse(E.Equal(stage, integer(i)), branch, body)
    action = compile_delegate(S.Action, E.IfThen(E.AndAlso(E.Equal(item(state, 1), integer(1)), gate), body))
    invoke = method(R.RhinoApp, 'InvokeOnUiThread', [S.Delegate, S.Array[S.Object]])
    parameter = E.Parameter(clr.GetClrType(S.Object), 'state')
    callback = compile_delegate(S.Threading.TimerCallback, block([call(None, invoke, [E.Constant(action, clr.GetClrType(S.Delegate)), E.Constant(S.Array[S.Object]([]))])]), [parameter])
    timer = S.Threading.Timer(callback, None, 500, 1500)
    try:
        returned = R.RhinoApp.RunScript(command, True)
    finally:
        state[1] = 0; timer.Dispose()
    if not state[0]: raise ValueError('owned 3DFace physical input was not requested')
    return bool(returned), dict(requests=int(state[0]), prompts=list(observed)[:state[0]], input='owned physical boundary click' if case.get('edge_pick') else 'owned physical Escape key')
