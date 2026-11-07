"""Audit ordered settings transitions, geometry and retained timeout diagnostics."""
from . import arc_blend_command as recipes
from .arc_blend_getter_evidence import capture_set, collect as prior_evidence
from .arc_blend_radius_evidence import finite
from .check_arc_blend_preferences import ROOT
FAMILIES=dict(arc_blend_preferences=list(recipes.PREFERENCES),arc_blend_preference_holdouts=list(recipes.PREFERENCE_HOLDOUTS),arc_blend_preference_cancellation=list(recipes.PREFERENCE_CANCELLATION))
DIAGNOSTIC='arc_blend_preference_holdouts/prefs_numeric_cancel'

def collect(proof):
    rows,captures,evidence=capture_set(proof,FAMILIES)
    *_,prior=prior_evidence(ROOT/'docs/arc-blend-getters-proof');evidence.update(prior)
    evidence.add(ROOT/'docs/arc-blend-getters-validation.json')
    workflows={};diagnostics={};picks=stations=0
    for key,v in rows.items():
        name=key.split('/',1)[1];trace=v['input_trace'][0]
        if v['prescribed_case']!=recipes.CASES[name]or not v['returned_idle']or trace['completed']!=trace['expected']or trace['expected']!=len(v['prescribed_case']['actions'])or 'Unknown command'in v['history']:raise ValueError('incomplete settings transition')
        if key==DIAGNOSTIC:
            if trace['errors']!=['owned ArcBlend input timeout']or v['success']or v['history_changed']:raise ValueError('timeout classification changed')
            diagnostics[key]=v
            continue
        if not finite(v['after'])or any(not o['native_valid']or o['selected']for o in v['after']):raise ValueError('invalid settings geometry')
        if trace['errors']or v['success']is not v['history_changed']or v['after']!=v['redo']:raise ValueError('incomplete settings transition')
        if not v['success']:
            if [dict(o,selected=0)for o in v['before']]!=v['after']:raise ValueError('cancellation changed geometry')
            if v['prescribed_case'].get('pending_redo')and not any(o.get('point')==[90.,91.,92.]for o in v['pending_redo']):raise ValueError('cancel lost pending redo')
        else:
            outputs=[o for o in v['after']if not o['attributes']['name']]
            if len(outputs)!=1:raise ValueError('missing settings output')
            if 'arc_segments'in outputs[0]:
                arcs=outputs[0]['arc_segments']
                if len(arcs)!=2 or any(len(o['samples'])!=33 for o in [outputs[0],*arcs]):raise ValueError('incomplete settings samples')
                stations+=99
        for click in trace['clicks']:
            if not click['motion_acknowledged']or not click['click_acknowledged']or not finite(click['ray'])or len(click['ray'])!=2:raise ValueError('invalid physical input receipt')
            picks+=1
        workflows[key]=v
    for family in FAMILIES:
        values=[(key.split('/',1)[1],v)for key,v in rows.items()if key.startswith(family+'/')]
        trim=join=False;radius=0.
        for name,v in values:
            # The first adjustment prompt exposes inherited Trim/Join state.
            first=v['history'].split('Press Enter when done',1)[1].split('\n',1)[0].split(')',1)[0]
            if ('Trim=Yes'in first)!=trim or (trim and ('Join=Yes'in first)!=join):raise ValueError('inherited option labels changed')
            pending=False
            for action in v['prescribed_case']['actions']:
                if action=='RadiusDifference':pending=True
                elif pending:
                    if isinstance(action,(float,int)):radius=action
                    elif action not in ('Enter','Cancel'):raise ValueError('invalid remembered radius recipe')
                    pending=False
                elif action=='Trim=Yes':trim=True
                elif action=='Trim=No':trim=False
                elif action=='Join=Yes':join=True
                elif action=='Join=No':join=False
            if name in ('prefs_radius_default','prefs_radius_after_cancel','prefs_numeric_default'):
                if 'RadiusDifference='+str(int(radius))not in v['history']:raise ValueError('native radius default changed')
    twice=workflows['arc_blend_preference_cancellation/prefs_numeric_twice']
    if 'RadiusDifference=5'not in twice['history']or twice['history'].count('_Cancel')!=2:raise ValueError('nested typed cancellation changed')
    if (len(workflows),len(diagnostics),stations)!=(25,1,1386):raise ValueError('settings coverage changed')
    return workflows,diagnostics,captures,picks,stations,evidence
