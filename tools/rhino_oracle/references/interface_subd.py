"""Mixed interface transitions with an explicit initial SubD appearance."""
def request():
    operations=[]
    for flat in [False,True]:
        for flags in range(8):
            for viewport in range(4):
                operations.append(dict(op='interface_commands',id='interface-subd-{}-{}-{}'.format(int(flat),flags,viewport),
                    subd_control_net=flat,grid_snap=bool(flags&1),osnap=bool(flags&2),smart_track=bool(flags&4),
                    active_viewport=viewport,display_modes=['Wireframe','Shaded','Ghosted','Wireframe'],
                    commands=['SubDDisplayToggle','Snap','SetDisplayMode Viewport=All Mode=Ghosted',
                        "'-_SubDDisplayToggle",'SetDisplayMode Mode=Shaded','DisableOsnap Toggle',
                        'SmartTrack Toggle','_SubDDisplayToggle','SubDDisplayToggle']))
    return dict(protocol_version=1,iterations=1,operations=operations)
