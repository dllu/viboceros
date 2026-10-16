"""Configure only the owned public Orient on Surface dialog via Win32 controls."""
def watch(options,configure=True):
    import ctypes,clr,System
    from ctypes import wintypes as W
    clr.AddReference('WindowsBase')
    from System.Windows.Threading import DispatcherTimer
    user=ctypes.windll.user32
    cb=ctypes.WINFUNCTYPE(W.BOOL,W.HWND,W.LPARAM)
    user.EnumWindows.argtypes=[cb,W.LPARAM];user.EnumChildWindows.argtypes=[W.HWND,cb,W.LPARAM]
    user.GetWindowThreadProcessId.argtypes=[W.HWND,ctypes.POINTER(W.DWORD)]
    user.GetWindowTextW.argtypes=[W.HWND,W.LPWSTR,ctypes.c_int]
    user.GetClassNameW.argtypes=[W.HWND,W.LPWSTR,ctypes.c_int]
    user.GetWindowRect.argtypes=[W.HWND,ctypes.POINTER(W.RECT)]
    user.IsWindowVisible.argtypes=[W.HWND];user.IsWindowEnabled.argtypes=[W.HWND]
    user.SetWindowTextW.argtypes=[W.HWND,W.LPCWSTR]
    user.SendMessageW.argtypes=[W.HWND,W.UINT,W.WPARAM,W.LPARAM];user.SendMessageW.restype=W.LPARAM
    pid=ctypes.windll.kernel32.GetCurrentProcessId();state=dict(applied=False,errors=[])
    normalize=lambda s:s.replace('&','').replace(' ','').lower()
    def owned(window):
        p=W.DWORD();user.GetWindowThreadProcessId(window,ctypes.byref(p));return p.value==pid and user.IsWindowVisible(window)
    def info(window):
        text=ctypes.create_unicode_buffer(1024);kind=ctypes.create_unicode_buffer(128);r=W.RECT()
        user.GetWindowTextW(window,text,1024);user.GetClassNameW(window,kind,128);user.GetWindowRect(window,ctypes.byref(r))
        return dict(handle=int(window),text=text.value,kind=kind.value,rect=[r.left,r.top,r.right,r.bottom],enabled=bool(user.IsWindowEnabled(window)),checked=int(user.SendMessageW(window,0xf0,0,0))if kind.value=='Button'else None)
    def children(window):
        rows=[]
        def visit(w,unused):
            if owned(w):rows.append(info(w))
            return True
        user.EnumChildWindows(window,cb(visit),0);return rows
    def click(row):user.SendMessageW(row['handle'],0xf5,0,0)
    def check(row,value):
        if bool(user.SendMessageW(row['handle'],0xf0,0,0))!=value:click(row)
    def find_button(rows,label):
        found=[r for r in rows if r['kind']=='Button'and normalize(r['text'])==normalize(label)]
        if len(found)!=1:raise ValueError('owned surface dialog button missing: '+label)
        return found[0]
    def tick(sender,args):
        if state.get('busy'):return
        windows=[]
        def visit(w,unused):
            if owned(w):windows.append(info(w))
            return True
        user.EnumWindows(cb(visit),0)
        dialogs=[w for w in windows if normalize(w['text'])=='orientonsurface']
        if not dialogs:return
        window=dialogs[0]['handle'];rows=children(window)
        if state['applied']:
            # Reject a validation warning in this owned test, then cancel the
            # original dialog. Never dismiss unrelated windows or continue silently.
            for other in windows:
                if other['handle']==window or other['kind']!='#32770':continue
                extra=children(other['handle']);buttons=[r for r in extra if r['kind']=='Button'and normalize(r['text'])=='ok']
                if len(buttons)==1 and any('number' in r['text'].lower()or 'greater' in r['text'].lower()or 'scale can not be zero' in r['text'].lower()for r in extra):
                    state['errors'].append([r['text']for r in extra]);click(buttons[0]);click(find_button(rows,'Cancel'));return
            return
        state['busy']=True;state['before']=rows
        try:
            if not configure:
                state['after']=rows;state['applied']=True;click(find_button(rows,'OK'));return
            prompts=sorted([r for r in rows if r['kind']=='Button'and normalize(r['text'])=='prompt'],key=lambda r:r['rect'][0])
            if len(prompts)!=2:raise ValueError('expected owned scale and rotation prompt controls')
            check(prompts[0],False);check(prompts[1],False)
            check(find_button(rows,'Uniform'),options.get('uniform',True));check(find_button(rows,'Rigid'),options.get('rigid',True))
            rows=children(window)
            edits=sorted([r for r in rows if 'edit' in r['kind'].lower()],key=lambda r:(r['rect'][0],r['rect'][1]))
            if len(edits)!=4:raise ValueError('expected four owned scale/angle fields: '+repr(edits))
            scales=options.get('axes',[options.get('scale',1)]*3)
            for row,value in zip(edits[:3]+edits[3:],list(scales)+[options.get('rotation',0)]):
                if not user.SetWindowTextW(row['handle'],repr(float(value))):raise ValueError('owned scale field rejected text')
            check(prompts[0],options.get('scale_prompt',False));check(prompts[1],options.get('rotation_prompt',False))
            state['after']=children(window);state['applied']=True
            click(find_button(rows,'OK'))
        except Exception as e:
            state['errors'].append(str(e));state['applied']=True
            cancel=[r for r in rows if r['kind']=='Button'and normalize(r['text'])=='cancel']
            if len(cancel)==1:click(cancel[0])
        finally:state['busy']=False
    timer=DispatcherTimer();timer.Interval=System.TimeSpan.FromMilliseconds(50);timer.Tick+=tick;timer.Start()
    return timer,state
