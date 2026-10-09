# -*- coding: utf-8 -*-
"""Observe the owned native restricted-member warning and expose its choice."""
import json
import os


def watch(token, choice):
    import ctypes,clr,System
    from ctypes import wintypes
    clr.AddReference('WindowsBase')
    from System.Windows.Threading import DispatcherTimer
    user=ctypes.windll.user32
    callback=ctypes.WINFUNCTYPE(wintypes.BOOL,wintypes.HWND,wintypes.LPARAM)
    user.EnumWindows.argtypes=[callback,wintypes.LPARAM]
    user.EnumChildWindows.argtypes=[wintypes.HWND,callback,wintypes.LPARAM]
    user.GetWindowThreadProcessId.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.DWORD)]
    user.GetWindowTextW.argtypes=[wintypes.HWND,wintypes.LPWSTR,ctypes.c_int]
    user.GetClassNameW.argtypes=[wintypes.HWND,wintypes.LPWSTR,ctypes.c_int]
    user.GetWindowRect.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.RECT)]
    user.IsWindowVisible.argtypes=[wintypes.HWND]
    path=os.path.join(os.path.dirname(os.path.abspath(__file__)),'block-edit-restricted.json')
    if os.path.isfile(path):os.remove(path)
    label={'release':'&Yes','restore':'&No','cancel':'Cancel'}[choice]
    def tick(sender,args):
        dialogs=[]
        def visit(window,unused):
            pid=wintypes.DWORD();user.GetWindowThreadProcessId(window,ctypes.byref(pid))
            text=ctypes.create_unicode_buffer(128);kind=ctypes.create_unicode_buffer(128)
            user.GetWindowTextW(window,text,128);user.GetClassNameW(window,kind,128)
            if pid.value==ctypes.windll.kernel32.GetCurrentProcessId() and user.IsWindowVisible(window) and kind.value=='#32770' and text.value.endswith('Block Edit'):dialogs.append(window)
            return True
        user.EnumWindows(callback(visit),0)
        if len(dialogs)!=1:return
        rows=[]
        def child(window,unused):
            text=ctypes.create_unicode_buffer(2048);kind=ctypes.create_unicode_buffer(128);rect=wintypes.RECT()
            user.GetWindowTextW(window,text,2048);user.GetClassNameW(window,kind,128);user.GetWindowRect(window,ctypes.byref(rect))
            rows.append((text.value,kind.value,[rect.left,rect.top,rect.right,rect.bottom]));return True
        user.EnumChildWindows(dialogs[0],callback(child),0)
        warning=any('hidden or locked objects' in text for text,kind,rect in rows)
        empty=any('must contain at least' in text and 'one object' in text for text,kind,rect in rows)
        if not warning and not empty:return
        marker=path+'.empty' if empty else path
        if os.path.isfile(marker):return
        wanted='OK' if empty else label
        buttons=[rect for text,kind,rect in rows if text.replace('&','')==wanted.replace('&','') and kind=='Button']
        if len(buttons)!=1:return
        l,t,r,b=buttons[0]
        value=dict(token=token,choice='empty' if empty else choice,point=[(l+r)/2.,(t+b)/2.])
        with open(marker+'.tmp','w') as stream:json.dump(value,stream)
        os.rename(marker+'.tmp',marker)
    timer=DispatcherTimer();timer.Interval=System.TimeSpan.FromMilliseconds(50);timer.Tick+=tick;timer.Start()
    return timer

def finish(timer):
    timer.Stop()
    path=os.path.join(os.path.dirname(os.path.abspath(__file__)),'block-edit-restricted.json')
    if os.path.isfile(path):os.remove(path)
    if os.path.isfile(path+'.empty'):os.remove(path+'.empty')
