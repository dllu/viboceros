"""Rejected-option diagnostics, excluded from point extraction qualification."""
DISCOVERY = {}
for picked in [False, True]:
    name = 'picked' if picked else 'preselected'
    DISCOVERY['target_layer_discovery_'+name] = dict(
        sources=[dict(kind='rational', mode=None, grips=[]),
                 dict(kind='surface', mode=None, grips=[])],
        whole=[0], order=[0, 1], output='Points', layer='TargetObject', layer_target=1,
        target_getter=picked)

ECHO = {
    'target_layer_echo_'+name: dict(DISCOVERY['target_layer_discovery_'+name], target_echo=True)
    for name in ['preselected', 'picked']
}

MENU = {
    'target_layer_menu': dict(DISCOVERY['target_layer_discovery_picked'],
                              target_echo=True, target_split=True)
}
