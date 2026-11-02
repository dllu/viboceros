"""Bounded public keystrokes answer only the owned direction selection getter."""
def start(R,S,doc,target,index,step):
    import clr
    clr.AddReference('WindowsBase')
    from System.Windows.Threading import DispatcherTimer
    timer=DispatcherTimer();timer.Interval=S.TimeSpan.FromMilliseconds(50)
    state=dict(target=str(target),target_index=index,sent=[],errors=[],ended=[])
    wanted='Dir' if step in ('UReverse','VReverse','SwapUV') else step
    def ended(sender,args):
        state['ended'].append(dict(name=str(args.CommandEnglishName),result=str(args.CommandResult)))
        if str(args.CommandEnglishName)==wanted:timer.Stop()
    R.Commands.Command.EndCommand+=ended
    started=S.DateTime.UtcNow;last=[started];busy=[False]
    def tick(sender,args):
        if busy[0]:return
        prompt=str(R.RhinoApp.CommandPrompt);phase=len(state['sent'])%3
        if (S.DateTime.UtcNow-started).TotalSeconds>30:
            timer.Stop();state['errors'].append('selection getter timeout: '+prompt)
            R.RhinoApp.SendKeystrokes('_Cancel',True);return
        if (S.DateTime.UtcNow-last[0]).TotalSeconds<.2:return
        if len(state['sent'])>=9:return
        getter=R.Input.Custom.GetObject.ActiveGetObject(doc)
        token=None
        if phase==0 and getter is not None and prompt.lower().startswith('select') and 'object' in prompt.lower():token='_SelID'
        elif phase==1 and 'object id to select' in prompt.lower():token=str(target)
        elif phase==2 and getter is not None and getter.ObjectCount>0 and prompt.lower().startswith('select'):token=''
        if token is None:return
        busy[0]=True
        try:
            state['sent'].append(dict(token=token,prompt=prompt));last[0]=S.DateTime.UtcNow
            R.RhinoApp.SendKeystrokes(token,True)
        except Exception as error:state['errors'].append(str(error));timer.Stop()
        finally:busy[0]=False
    timer.Tick+=tick;timer.Start();closed=[False]
    def stop():
        if closed[0]:return
        closed[0]=True
        timer.Stop()
        R.Commands.Command.EndCommand-=ended
    def finish():
        stop()
        if state['errors'] or len(state['sent'])%3==1:raise ValueError('owned ShowDir input failed: '+str(state)+'; history='+str(R.RhinoApp.CommandHistoryWindowText)[-1800:])
        return state
    finish.stop=stop
    return finish
