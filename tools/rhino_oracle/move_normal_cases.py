"""Normal reference recipes prescribed before native measurement."""
import copy
from .translation_cases import SOURCES


def targets():
    values = []
    def add(label,kind,points,aim,flip=False,view='Top'):
        values.append((label,dict(kind=kind,points=points,aim=aim,flip=flip,view=view)))
    add('line','line',[[0,0,0],[6,2,0]],[3,1,0])
    add('line-reversed','line',[[0,0,0],[6,2,0]],[3,1,0],True)
    add('line-spatial','line',[[0,0,0],[6,2,4]],[3,1,2],view='Perspective')
    add('circle','circle',[[0,0,0]],[3,0,0])
    add('circle-reversed','circle',[[0,0,0]],[3,0,0],True)
    add('arc','arc',[[3,0,0],[0,3,0],[-3,0,0]],[0,3,0])
    add('polyline','polyline',[[0,0,0],[6,0,0],[6,6,2]],[3,0,0])
    for kind in ('surface','brep'):
        for flip in (False,True):
            add(kind+('-flip' if flip else ''),kind,[[0,0,0],[6,0,0],[6,6,4],[0,6,4]],[3,3,2],flip)
    add('warped','surface',[[0,0,0],[6,0,0],[6,6,4],[0,6,0]],[3,3,1])
    return values


def request():
    operations = []
    def add(label,target,inputs,post=False):
        row = dict(op='transform_copy_command',id='move-normal-'+label,command='Move',
            sources=copy.deepcopy(SOURCES),grouped=True,inputs=inputs,finish='Cancel',undo_redo=True,sel_last=True,
            normal_target=copy.deepcopy(target))
        if post: row['source_selection'] = [2,0,'Enter']
        else: row['selected'] = [2,0]
        operations.append(row)
    for label,target in targets():
        base = 'w'+','.join(str(value) for value in target['aim'])
        for post in (False,True):
            for suffix,destination in [('positive','3.5'),('negative','-2'),('coordinate','w8,-3,9')]:
                add(label+('-post-' if post else '-pre-')+suffix,target,['Normal','NormalTarget',base,destination],post)
    for label,target in [row for row in targets() if row[0] in ('circle','surface','brep-flip')]:
        base = 'w'+','.join(str(value) for value in target['aim'])
        for post in (False,True):
            for suffix,inputs in [('cancel-base',['Normal','NormalTarget']),('enter-base',['Normal','NormalTarget','Enter']),
                                 ('cancel-destination',['Normal','NormalTarget',base]),('zero',['Normal','NormalTarget',base,'0'])]:
                add(label+('-post-' if post else '-pre-')+suffix,target,inputs,post)
    circle = dict(targets())['circle']; surface = dict(targets())['surface']
    add('default-seed',circle,['Normal','NormalTarget','w3,0,0','-2'])
    add('default-circle',circle,['Normal','NormalTarget','w3,0,0','Enter'])
    add('default-surface',surface,['Normal','NormalTarget','w3,3,2','Enter'])
    for label,target in [row for row in targets() if row[0] in ('circle','surface','warped')]:
        add(label+'-off-reference',target,['Normal','NormalTarget','w8,-1,7','2.5'])
    return dict(protocol_version=1,iterations=1,operations=operations)


def edges_request():
    cases=[]; base=request()['operations'][0]
    for label,target in targets():
        if label not in ('circle','arc','surface','surface-flip','brep-flip','warped'): continue
        for view in ('Top','Perspective'):
            row=copy.deepcopy(base); row['id']='move-normal-mouse-'+label+'-'+view
            row['normal_target']=copy.deepcopy(target); row['normal_target']['view']=view
            row['inputs']=['Normal','NormalTarget','NormalBase','3.5']; cases.append(row)
    for label,target in targets():
        if label not in ('surface','brep'): continue
        for ignore in ('Yes','No'):
            for post in (False,True):
                row=copy.deepcopy(base); row['id']='move-normal-ignore-'+label+'-'+ignore+('-post' if post else '-pre')
                row['normal_target']=copy.deepcopy(target)
                row['inputs']=['Normal','IgnoreTrims='+ignore,'NormalTarget','w8,-1,7','-2']
                if post: row.pop('selected'); row['source_selection']=[2,0,'Enter']
                cases.append(row)
    return dict(protocol_version=1,iterations=1,operations=cases)


def trims_request():
    cases=[]; base=request()['operations'][0]
    target=dict(targets())['warped']; target['kind']='trimmed'; target['aim']=[1.5,3,.5]
    for ignore in ('No','Yes'):
        for post in (False,True):
            for destination in ('3.5','w8,-3,9'):
                row=copy.deepcopy(base)
                row['id']='move-normal-trim-'+ignore+('-post-' if post else '-pre-')+('distance' if destination=='3.5' else 'coordinate')
                row['normal_target']=copy.deepcopy(target)
                row['inputs']=['Normal','IgnoreTrims='+ignore,'NormalTarget','w5,3,2',destination]
                if post: row.pop('selected'); row['source_selection']=[2,0,'Enter']
                cases.append(row)
    return dict(protocol_version=1,iterations=1,operations=cases)


def defaults_request():
    cases=[]; base=request()['operations'][0]; references=dict(targets())
    for label,reference,base_point,tail in [
        ('seed','circle','w3,0,0',['-2']),
        ('circle-positive','circle','w3,0,0',['Enter','w8,-3,9']),
        ('circle-negative','circle','w3,0,0',['Enter','w-8,-3,9']),
        ('surface-seed','surface','w3,3,2',['2.5']),
        ('surface-positive','surface','w3,3,2',['Enter','w8,-3,9']),
        ('surface-negative','surface','w3,3,2',['Enter','w8,9,-3']),
        ('zero-seed','circle','w3,0,0',['0']),
        ('zero-destination','circle','w3,0,0',['Enter','w8,-3,9']),
    ]:
        row=copy.deepcopy(base);row['id']='move-normal-defaults-'+label
        row['normal_target']=copy.deepcopy(references[reference]);row['inputs']=['Normal','NormalTarget',base_point]+tail
        cases.append(row)
    return dict(protocol_version=1,iterations=1,operations=cases)
