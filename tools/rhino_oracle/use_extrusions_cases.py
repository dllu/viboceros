"""Application-wide extrusion policy, native input and model-history witnesses."""


def cases():
    return [dict(op='use_extrusions',id=initial+'_'+answer,initial=initial,answer=answer)
            for initial in ('Extrusion','Polysurface')
            for answer in ('Extrusion','Polysurface','Enter','Cancel')]


def validate(op):
    if not isinstance(op,dict) or op not in cases():raise ValueError('fixed owned UseExtrusions recipe required')
