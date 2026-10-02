"""Source-only component preselection and empty-space rectangle experiments."""
import copy
import json

from .remove_holes_cases import request as geometry_request


def request():
    base = next(op['source'] for op in geometry_request()['operations'] if op['id'] == 'two-holes-all')
    operations = []
    def add(identifier, all_value, pick, components, keep=False, finish='Enter', **extra):
        operations.append(dict(op='untrim_holes_command', id=identifier,
            sources=[dict(brep=copy.deepcopy(base))], all=all_value, components=components,
            maximum_edge_length=0., keep_trim_objects=keep, pick=pick,
            finish=finish, undo_redo=True, trace_components=True, **extra))
    for all_value in (False, True):
        for kind, components in [('edge', [[0,1]]), ('edge', [[0,1],[0,2]]), ('face', [[0,0]])]:
            add('pre-all-%d-%s-%d' % (all_value, kind, len(components)), all_value,
                'preselect', components, preselect_kind=kind)
    windows = [('first', [[.5,2.5,0],[5.5,5.5,0]]),
               ('both', [[.5,2.5,0],[8.5,8.5,0]]),
               ('cross', [[9.5,4.5,0],[4.5,3.5,0]]),
               ('whole', [[.5,.5,0],[9.5,9.5,0]])]
    for all_value in (False, True):
        for sub in (False, True):
            for label, window in windows:
                add('window-all-%d-sub-%d-%s' % (all_value, sub, label), all_value,
                    'window', [], keep=True, window=window, window_subobjects=sub)
    for all_value in (False, True):
        add('window-cancel-all-%d' % all_value, all_value, 'window', [], keep=True,
            finish='Cancel', window=windows[0][1], window_subobjects=True)
    for sub in (False, True):
        add('window-split-sub-%d' % sub, False, 'window', [], keep=True,
            window=windows[0][1], window_subobjects=sub)
        operations[-1]['sources'][0]['brep']['splits'] = [[1,[1.,2.,3.]]]
        operations[-1]['sources'][0]['brep']['source']['boundaries'].pop()
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == '__main__': print(json.dumps(request(), indent=2))
