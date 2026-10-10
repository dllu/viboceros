"""Owned public Divide invocations capturing numeric and option defaults."""
import re

SEED = '_Divide _Split=_No _MarkEnds=_Yes _GroupOutput=_Yes 6 _Enter'
LENGTH = '_Divide _Split=_No _MarkEnds=_No _GroupOutput=_No _Length 2.5 _Enter'
CHORD = '_Divide _Split=_No _MarkEnds=_Yes _GroupOutput=_No _EqualChordLength 3 _Enter'
SPECS = {
    'count_repeat': [
        SEED,
        '_Divide _Enter',
    ],
    'length_then_count': [
        SEED,
        LENGTH,
        '_Divide _Enter',
        '_Divide _Length _Enter',
    ],
    'chord_then_count': [
        SEED,
        CHORD,
        '_Divide _Enter',
        '_Divide _EqualChordLength _Enter',
    ],
    'length_chord_memory': [
        SEED,
        LENGTH,
        CHORD,
        '_Divide _Length _Enter',
        '_Divide _EqualChordLength _Enter',
    ],
    'cancel_options': [
        SEED,
        '_Divide _GroupOutput=_No _Split=_Yes _Cancel',
        '_Divide _Enter',
    ],
    'cancel_points': [
        SEED,
        '_Divide _MarkEnds=_No _GroupOutput=_No _Cancel',
        '_Divide _Enter',
    ],
    'cancel_remainder': [
        SEED,
        '_Divide _Split=_Yes _Length _DeleteRemainder=_Yes 2.5 _Enter',
        '_Divide _Length _DeleteRemainder=_No _Cancel',
        '_Divide _Length _Enter',
    ],
    'cancel_mode': [
        SEED,
        LENGTH,
        '_Divide _EqualChordLength _Cancel',
        '_Divide _Enter',
        '_Divide _Length _Enter',
    ],
}


def validate(op):
    if (
        set(op) != {'op', 'id', 'case'}
        or op.get('op') != 'divide_preferences'
        or not isinstance(op.get('id'), (str, type(u'')))
        or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
        or not isinstance(op.get('case'), (str, type(u'')))
        or op.get('case') not in SPECS
    ):
        raise ValueError('invalid Divide preferences recipe')


def run(op, host):
    validate(op)
    Rhino, System = (host['Rhino'], host['System'])
    if Rhino.Commands.Command.InCommand():
        raise ValueError('Divide preferences require idle execution')
    doc, G = (Rhino.RhinoDoc.ActiveDoc, Rhino.Geometry)
    if list(doc.Objects):
        raise ValueError('Divide preferences require an empty owned document')
    from join_probe import observe_command
    records = []
    for macro in SPECS[op['case']]:
        source = doc.Objects.AddLine(G.Point3d(0, 0, 0), G.Point3d(20.7, 0, 0))
        if source == System.Guid.Empty:
            raise ValueError('Divide source insertion failed')
        doc.Objects.UnselectAll()
        doc.Objects.Select(source)
        group_map = {}

        def snapshot():
            rows = []
            for obj in sorted(list(doc.Objects), key=lambda o: int(o.RuntimeSerialNumber)):
                if obj.Id == source:
                    continue
                g, a = (obj.Geometry, obj.Attributes)
                groups = []
                for index in a.GetGroupList() or []:
                    if index not in group_map:
                        group_map[index] = len(group_map)
                    groups.append(group_map[index])
                if isinstance(g, G.Point):
                    kind = 'point'
                    points = [[float(g.Location.X), float(g.Location.Y), float(g.Location.Z)]]
                    domain = None
                else:
                    kind = 'curve'
                    domain = [float(g.Domain.T0), float(g.Domain.T1)]
                    points = []
                    for i in range(17):
                        p = g.PointAt(g.Domain.T0 + (g.Domain.T1 - g.Domain.T0) * i / 16.0)
                        points.append([float(p.X), float(p.Y), float(p.Z)])
                rows.append(dict(kind=kind, points=points, domain=domain, groups=groups))
            source_object = doc.Objects.FindId(source)
            return dict(
                outputs=rows,
                input_count=int(source_object is not None and not source_object.IsDeleted),
                group_count=len(group_map),
            )

        history = Rhino.RhinoApp.CommandHistoryWindowText
        try:
            success, after, events = observe_command(
                Rhino.Commands.Command, 'Divide',
                lambda: Rhino.RhinoApp.RunScript(macro, True), snapshot, lambda: [], True,
            )
            active = bool(Rhino.Commands.Command.InCommand())
            records.append(dict(
                macro=macro, succeeded=success, after=after, events=events, active=active,
                history=Rhino.RhinoApp.CommandHistoryWindowText[len(history):],
            ))
        finally:
            if Rhino.Commands.Command.InCommand():
                Rhino.RhinoApp.RunScript('!', False)
            for obj in list(doc.Objects):
                doc.Objects.Delete(obj.Id, True)
    return (dict(steps=records), 0)
