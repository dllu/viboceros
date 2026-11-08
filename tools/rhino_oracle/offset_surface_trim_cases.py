"""Authored open trimmed sheets for non-Loose support-surface offsets."""
import copy
if __package__:
    from .offset_surface_tolerance_cases import case, BASE
    from .offset_surface_loose_trim_cases import CURVED_TRIM, boundary
else:
    from offset_surface_tolerance_cases import case, BASE
    from offset_surface_loose_trim_cases import CURVED_TRIM, boundary

REVERSED = copy.deepcopy(CURVED_TRIM)
REVERSED['reversed'] = True
SHIFTED = copy.deepcopy(CURVED_TRIM)
for net in [SHIFTED['surface']]+[b['curve'] for b in SHIFTED['boundaries']]:
    for control in net['control_points']:
        control['point'][0] += 10.
        control['point'][2] += 2.
NARROW = copy.deepcopy(CURVED_TRIM)
NARROW['boundaries'] = [boundary(.15,.85), boundary(.4,.55,True)]

def trimmed(actions, **kwargs):
    return case(actions, curves=[copy.deepcopy(CURVED_TRIM)], **kwargs)

TRIM_OFFSETS = {
    'offset_te_negative': trimmed([-.3,'Tolerance',.0001,'Enter']),
    'offset_te_reversed': case(['Tolerance',.0001,'Enter'],curves=[REVERSED]),
    'offset_te_both': trimmed(['BothSides=Yes','Tolerance',.0001,'Enter']),
    'offset_te_fine': trimmed(['Tolerance',.000001,'Enter']),
    'offset_te_delete_postselect': dict(trimmed([]), preselected=False,
        actions=[{'curve':0},'Enter']+BASE+['DeleteInput=Yes','Tolerance',.0001,'Enter']),
    'offset_te_batch': case(['Tolerance',.0001,'Enter'],curves=[CURVED_TRIM,SHIFTED]),
    'offset_te_narrow': case(['Tolerance',.0001,'Enter'],curves=[NARROW]),
    'offset_te_cancel': trimmed(['Tolerance',.0001,'Cancel']),
}

# DeleteInput is only offered while Solid=Yes. Set it before returning to No.
TRIM_FINAL = {
    'offset_tef_delete_postselect': dict(trimmed([]), preselected=False,
        actions=[{'curve':0},'Enter']+
        [('DeleteInput=Yes' if a=='DeleteInput=No' else a) for a in BASE]+
        ['Tolerance',.0001,'Enter']),
}
