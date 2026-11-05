# coding: utf-8
"""Owned block/extrusion type filters, names, visibility and selection history."""


def cases():
    rows=[]
    for command in ('SelBlockInstance','SelMirroredBlocks','SelExtrusion'):
        for variant in ('plain','grouped','preselected','restricted','hidden_layer','locked_layer','no_matches'):
            rows.append(dict(op='type_selection',id=command+'_'+variant,command=command,variant=variant))
    for name in ('Bolt A','bolt a','Bolt *','Bolt ?','Nested','Other',u'部品 A','Missing'):
        rows.append(dict(op='type_selection',id='named_'+str(len(rows)),command='SelBlockInstanceNamed',variant='plain',name=name))
    for variant in ('grouped','preselected','restricted','hidden_layer','locked_layer','no_matches'):
        rows.append(dict(op='type_selection',id='named_'+variant,command='SelBlockInstanceNamed',variant=variant,name='Bolt A'))
    return rows


def validate(op):
    if not isinstance(op,dict) or op not in cases()+empty_cases() or ('preselected' in op and type(op['preselected']) is not bool):
        raise ValueError('fixed owned type-selection recipe required')


def empty_cases():
    return [dict(op='type_selection',id=command+'_'+variant+'_preselected',command=command,
                 variant=variant,preselected=True,**({'name':'Bolt A'} if command=='SelBlockInstanceNamed' else {}))
            for command in ('SelBlockInstance','SelMirroredBlocks','SelExtrusion','SelBlockInstanceNamed')
            for variant in ('no_matches','restricted')]
