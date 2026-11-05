"""Owned deletion/Undo recipes, including explicit membership removal controls."""


def cases():
    rows = []
    kinds = ['brep', 'mesh', 'subd']
    for flat in (False, True):
        appearance = 'flat' if flat else 'smooth'
        for deleted in ([0], [1], [2], [0, 1, 2]):
            name = '_'.join(kinds[i] for i in deleted)
            rows.append(dict(op='edge_analysis_lifetime', id='mixed_'+name+'_'+appearance,
                             flat=flat, analyzed=[0, 1, 2], removed=[], deleted=deleted))
        rows.append(dict(op='edge_analysis_lifetime', id='removed_mesh_'+appearance,
                         flat=flat, analyzed=[0, 1, 2], removed=[1], deleted=[1]))
    for i, kind in enumerate(kinds):
        rows.append(dict(op='edge_analysis_lifetime', id='only_'+kind,
                         flat=False, analyzed=[i], removed=[], deleted=[i]))
    return rows


def validate(op):
    if (not isinstance(op, dict) or op not in cases() + runtime_cases() + selection_cases() + mode_cases() or type(op.get('flat')) is not bool
            or any(k in op and type(op[k]) is not bool for k in ('runtime','skip_zoom'))
            or any(type(i) is not int for name in ('analyzed', 'removed', 'deleted', 'targets') for i in op.get(name, []))):
        raise ValueError('fixed owned edge-analysis lifetime recipe required')


def runtime_cases():
    rows = []
    for row in cases():
        item = dict(row, id='runtime_'+row['id'], runtime=True, edit='Delete')
        item['targets'] = item.pop('deleted')
        rows.append(item)
    for edit in ('Move', 'Flip'):
        for flat in (False, True):
            for targets in ([0], [1], [2], [0, 1, 2]):
                name = '_'.join(['brep', 'mesh', 'subd'][i] for i in targets)
                rows.append(dict(op='edge_analysis_lifetime', id=edit.lower()+'_'+name+'_'+str(int(flat)),
                                 flat=flat, analyzed=[0, 1, 2], removed=[], targets=targets,
                                 runtime=True, edit=edit))
    return rows


def selection_cases():
    rows = []
    for targets in ([0], [1], [2], [0, 1, 2]):
        for mode in ('Zoom', 'Naked', 'All'):
            rows.append(dict(op='edge_analysis_lifetime', id='select_'+'_'.join(map(str, targets))+'_'+mode,
                             flat=False, analyzed=[0, 1, 2], removed=[], targets=targets,
                             runtime=True, edit='Select', panel_mode=mode))
    for mode in ('Zoom', 'Naked'):
        rows.append(dict(op='edge_analysis_lifetime', id='outside_'+mode,
                         flat=False, analyzed=[0], removed=[], targets=[1, 2],
                         runtime=True, edit='Select', panel_mode=mode))
    return rows


def mode_cases():
    rows = []
    for initial in ('All', 'Naked', 'NonManifold'):
        for target in ('All', 'Naked', 'NonManifold'):
            rows.append(dict(op='edge_analysis_lifetime', id='mode_'+initial+'_'+target,
                             flat=False, analyzed=[0, 1, 2], removed=[], targets=[0, 1, 2],
                             runtime=True, edit='Select', initial_mode=initial, panel_mode=target, skip_zoom=True))
    for initial, steps in [('All', ['Naked', 'All', 'Naked']), ('Naked', ['All', 'Naked', 'All', 'Naked'])]:
        rows.append(dict(op='edge_analysis_lifetime', id='repeat_'+initial,
                         flat=False, analyzed=[0, 1, 2], removed=[], targets=[0, 1, 2],
                         runtime=True, edit='Select', initial_mode=initial, panel_mode='Naked',
                         mode_sequence=steps, skip_zoom=True))
    return rows
