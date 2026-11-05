"""Owned source-order and selection-route matrix for mixed edge diagnostics."""
from itertools import permutations


def cases():
    rows = []
    workflows = [('ZoomNaked', ['All', 'Mark']),
                 ('ZoomNaked', ['Mark', 'Next', 'Next', 'Next', 'Next', 'Mark', 'Previous', 'Mark'])]
    for order in permutations(('brep', 'mesh', 'subd')):
        for pre in (False, True):
            for flat in (False, True):
                for i, (command, actions) in enumerate(workflows):
                    rows.append(dict(op='edge_analysis_mixed', id='_'.join(order)+'_'+str(int(pre))+'_'+str(int(flat))+'_'+str(i),
                                     order=list(order), preselect=pre, flat=flat, grouped=False,
                                     extra_point=False, command=command, actions=actions))
            rows.append(dict(op='edge_analysis_mixed', id='_'.join(order)+'_show_'+str(int(pre)),
                             order=list(order), preselect=pre, flat=False, grouped=False,
                             extra_point=False, command='ShowEdges', actions=[]))
    for extra in (False, True):
        for pre in (False, True):
            for i, (command, actions) in enumerate([('ShowEdges', [])]+workflows):
                rows.append(dict(op='edge_analysis_mixed', id='group_'+str(int(extra))+'_'+str(int(pre))+'_'+str(i),
                                 order=['subd', 'mesh', 'brep'], preselect=pre, flat=True, grouped=True,
                                 extra_point=extra, command=command, actions=actions))
    return rows


def validate(op):
    if (not isinstance(op, dict) or op not in cases() + membership_cases() + panel_cases()
            or any(k in op and type(op[k]) is not bool for k in ('membership', 'panel_only', 'sequential'))
            or any(type(op.get(k)) is not bool for k in ('preselect', 'flat', 'grouped', 'extra_point'))):
        raise ValueError('fixed owned mixed edge-analysis recipe required')


def membership_cases():
    return [dict(op='edge_analysis_mixed',id='membership_'+'_'.join(order),order=list(order),
                 preselect=True,flat=False,grouped=False,extra_point=False,
                 command='ShowEdges',actions=[],membership=True)
            for order in permutations(('brep','mesh','subd'))]


def panel_cases():
    rows = [dict(op='edge_analysis_mixed', id='panel_'+'_'.join(order), order=list(order),
                 preselect=True, flat=False, grouped=False, extra_point=False,
                 command='ShowEdges', actions=[], membership=True, panel_only=True)
            for order in permutations(('brep', 'mesh', 'subd'))]
    for order in [('mesh', 'brep', 'subd'), ('mesh', 'subd', 'brep')]:
        rows.append(dict(op='edge_analysis_mixed', id='sequential_'+'_'.join(order), order=list(order),
                         preselect=True, flat=False, grouped=False, extra_point=False,
                         command='ShowEdges', actions=[], membership=True, sequential=True))
    return rows
