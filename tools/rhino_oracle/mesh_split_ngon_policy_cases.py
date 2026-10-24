"""Existing logical polygons across discard and CreateNgons policies."""
from copy import deepcopy


def cases(channels):
    decorated = channels['mesh_split_channels_existing_ngon']['sources'][0]
    result = {}
    for name, recipe, create, discard in [
            ('disjoint_keep_no', 'disjoint_keep', False, False),
            ('disjoint_discard_no', 'disjoint_discard', False, True),
            ('partial_keep_no', 'partial_keep', False, False),
            ('intersected_keep_yes', 'existing_ngon', True, True)]:
        value = deepcopy(channels['mesh_split_channels_' + recipe])
        value['sources'][0] = deepcopy(decorated)
        value['arguments'] = '_CreateNgons=_{0} _DiscardUnsplitMeshes=_{1}'.format(
            'Yes' if create else 'No', 'Yes' if discard else 'No')
        result['mesh_split_ngon_policy_' + name] = value
    return result
