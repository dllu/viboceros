"""Read-only public pick queries and managed physical-click observations."""
from contextlib import contextmanager


@contextmanager
def observing_picks(host, view, capacity, enabled):
    trace = PickTrace(host, view, capacity) if enabled else None
    try:
        if trace is not None:
            trace.start()
        yield trace
    finally:
        if trace is not None:
            trace.close()


class PickTrace:
    def __init__(self, host, view, capacity):
        import clr
        from System.Linq.Expressions import Expression as E, ParameterExpression
        from managed_mouse_callback import create
        S, R = host['System'], host['Rhino']
        self.host, self.view, self.expected = host, view, []
        self.count = S.Array[S.Int32]([0])
        self.pixels = S.Array[S.Int32]([0] * (2 * capacity))
        self.prompts = S.Array[S.String]([None] * capacity)
        self.views = S.Array[S.Guid]([S.Guid.Empty] * capacity)
        self.motion = S.Array[S.Int32]([-1, -1])
        self.prior_motion = S.Array[S.Int32]([-1] * (2 * capacity))

        def handler(argument, delegate):
            event = E.Parameter(argument, 'event')
            array = lambda values: S.Array[E](values)
            integer = lambda value: E.Constant(S.Int32(value))
            item = lambda values, index: E.ArrayAccess(E.Constant(values), array([index]))
            index = item(self.count, integer(0))
            pixel = E.Property(event, 'ViewportPoint')
            viewport = E.Property(E.Property(E.Property(event, 'View'), 'ActiveViewport'), 'Id')
            prompt = E.Property(None, clr.GetClrType(R.RhinoApp).GetProperty('CommandPrompt'))
            actions = [
                E.Assign(item(self.pixels, E.Multiply(index, integer(2))), E.Property(pixel, 'X')),
                E.Assign(item(self.pixels, E.Add(E.Multiply(index, integer(2)), integer(1))), E.Property(pixel, 'Y')),
                E.Assign(item(self.prompts, index), prompt),
                E.Assign(item(self.views, index), viewport),
                E.Assign(item(self.prior_motion, E.Multiply(index, integer(2))), item(self.motion, integer(0))),
                E.Assign(item(self.prior_motion, E.Add(E.Multiply(index, integer(2)), integer(1))), item(self.motion, integer(1))),
                E.Assign(index, E.Add(index, integer(1))), E.Empty(),
            ]
            body = E.IfThen(E.LessThan(index, integer(capacity)), E.Block(array(actions)))
            return E.Lambda(delegate, body, S.Array[ParameterExpression]([event])).Compile()

        self.callback = create(handler, host, 'OnMouseDown')
        def motion_handler(argument, delegate):
            event = E.Parameter(argument, 'event')
            pixel = E.Property(event, 'ViewportPoint')
            actions = [E.Assign(E.ArrayAccess(E.Constant(self.motion), S.Array[E]([E.Constant(S.Int32(i))])),
                                E.Property(pixel, axis)) for i, axis in enumerate(['X', 'Y'])]
            return E.Lambda(delegate, E.Block(S.Array[E](actions+[E.Empty()])),
                            S.Array[ParameterExpression]([event])).Compile()
        self.motion_callback = create(motion_handler, host, 'OnEndMouseMove')

    def start(self):
        self.motion_callback.Enabled = True
        self.callback.Enabled = True

    def query(self, point, x, y, source_ids, query_objects=True):
        R, vp = self.host['Rhino'], self.view.ActiveViewport
        context = R.Input.Custom.PickContext()
        refs = []
        xyz = lambda p: [float(p.X), float(p.Y), float(p.Z)]
        try:
            context.View = self.view
            context.PickStyle = R.Input.Custom.PickStyle.PointPick
            context.PickMode = R.Input.Custom.PickMode.Shaded
            context.PickGroupsEnabled = False
            ok, ray = vp.GetFrustumLine(x, y)
            if not ok:
                raise ValueError('owned trim pick ray unavailable')
            context.PickLine = ray
            context.SetPickTransform(vp.GetPickTransform(x, y))
            context.UpdateClippingPlanes()
            refs = list(R.RhinoDoc.ActiveDoc.Objects.PickObjects(context) or []) if query_objects else []
            hits = []
            for ref in refs:
                picked = ref.SelectionPoint()
                hits.append(dict(source=source_ids.index(ref.ObjectId) if ref.ObjectId in source_ids else None,
                                 point=xyz(picked) if picked.IsValid else None))
            self.expected.append(dict(point=xyz(point), pixel=[x, y], viewport=str(vp.Id),
                                      ray=[xyz(ray.From), xyz(ray.To)], hits=hits))
        finally:
            failures = []
            for ref in refs:
                try:
                    ref.Dispose()
                except Exception as error:
                    failures.append(str(error))
            try:
                context.Dispose()
            except Exception as error:
                failures.append(str(error))
            if failures:
                raise ValueError('owned pick reference cleanup failed: '+str(failures))

    def close(self):
        failures = []
        for callback in [self.callback, self.motion_callback]:
            try:
                callback.Enabled = False
            except Exception as error:
                failures.append(str(error))
        if failures:
            raise ValueError('owned pick callback cleanup failed: '+str(failures))

    def result(self):
        return dict(expected=self.expected, actual=[dict(
            pixel=[int(self.pixels[2*i]), int(self.pixels[2*i+1])],
            prior_motion=[int(self.prior_motion[2*i]), int(self.prior_motion[2*i+1])],
            viewport=str(self.views[i]), prompt=self.prompts[i]) for i in range(int(self.count[0]))])
