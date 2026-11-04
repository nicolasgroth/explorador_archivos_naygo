# Naygo — prueba sintética de subclass y timeout entre hilos Windows.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT
# No usa Naygo ni demuestra hit-test, integridad de destino o accesibilidad Slint.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -ReferencedAssemblies System.Windows.Forms -TypeDefinition @'
using System;
using System.Threading;
using System.Collections.Concurrent;
using System.Runtime.InteropServices;
using System.Windows.Forms;
public class TimeoutProbe : NativeWindow {
    [DllImport("user32.dll", SetLastError=true)]
    static extern IntPtr SendMessageTimeout(IntPtr h, uint m, IntPtr w, IntPtr l, uint flags, uint ms, out UIntPtr result);
    [DllImport("user32.dll")]
    static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    delegate IntPtr SubclassProc(IntPtr h, uint msg, IntPtr w, IntPtr l, UIntPtr id, UIntPtr data);
    [DllImport("comctl32.dll", SetLastError=true)]
    static extern bool SetWindowSubclass(IntPtr h, SubclassProc callback, UIntPtr id, UIntPtr data);
    [DllImport("comctl32.dll", SetLastError=true)]
    static extern bool RemoveWindowSubclass(IntPtr h, SubclassProc callback, UIntPtr id);
    [DllImport("comctl32.dll")]
    static extern IntPtr DefSubclassProc(IntPtr h, uint msg, IntPtr w, IntPtr l);
    const int Ping = 0x8001, Slow = 0x8002, Reenter = 0x8003;
    bool busy;
    static readonly ManualResetEvent slowDone = new ManualResetEvent(false);
    static readonly ConcurrentDictionary<int, string> requests = new ConcurrentDictionary<int, string>();
    static bool lateResponseDelivered;
    public static void Run() {
        var ready = new ManualResetEvent(false); IntPtr hwnd = IntPtr.Zero;
        var ui = new Thread(() => {
            var win = new TimeoutProbe(); win.CreateHandle(new CreateParams()); hwnd=win.Handle;
            SubclassProc callback=(h,m,w,l,id,data) => {
                if(m==Ping) return (IntPtr)(win.busy ? 99 : 42);
                if(m==Reenter) {win.busy=true; UIntPtr innerResult; SendMessageTimeout(h,Ping,IntPtr.Zero,IntPtr.Zero,0x22,1,out innerResult); win.busy=false; return (IntPtr)(long)innerResult.ToUInt64();}
                if(m==Slow) {Thread.Sleep(250); string request; lateResponseDelivered=requests.TryGetValue((int)w,out request); slowDone.Set(); return (IntPtr)7;}
                return DefSubclassProc(h,m,w,l);
            };
            if(!SetWindowSubclass(hwnd,callback,(UIntPtr)1,UIntPtr.Zero)) throw new Exception("subclass install failed");
            ready.Set(); Application.Run();
            if(!RemoveWindowSubclass(hwnd,callback,(UIntPtr)1)) throw new Exception("subclass remove failed");
            win.DestroyHandle(); GC.KeepAlive(callback);
        });
        ui.SetApartmentState(ApartmentState.STA); ui.IsBackground=true; ui.Start();
        if(!ready.WaitOne(5000)) throw new Exception("window did not start");
        UIntPtr response;
        var ok=SendMessageTimeout(hwnd,Ping,IntPtr.Zero,IntPtr.Zero,0x22,1000,out response);
        if(ok==IntPtr.Zero || response.ToUInt64()!=42) throw new Exception("ping failed");
        var nested=SendMessageTimeout(hwnd,Reenter,IntPtr.Zero,IntPtr.Zero,0x22,1000,out response);
        if(nested==IntPtr.Zero || response.ToUInt64()!=99) throw new Exception("reentrancy guard failed");
        var timer=System.Diagnostics.Stopwatch.StartNew();
        requests[5]="pending";
        var slow=SendMessageTimeout(hwnd,Slow,(IntPtr)5,IntPtr.Zero,0x22,30,out response);
        timer.Stop();
        string removed; requests.TryRemove(5, out removed);
        if(slow!=IntPtr.Zero || timer.ElapsedMilliseconds>200) throw new Exception("timeout failed");
        if(slowDone.WaitOne(0)) throw new Exception("slow callback already finished");
        if(!slowDone.WaitOne(2000)) throw new Exception("slow callback did not finish");
        if(lateResponseDelivered) throw new Exception("late response delivered");
        Console.WriteLine("subclass=installed; ping=42; reentrancy=rejected; timeout_ms="+timer.ElapsedMilliseconds+"; callback_completed_after_timeout=true; late_response_discarded=true");
        PostMessage(hwnd,0x0010,IntPtr.Zero,IntPtr.Zero);
        if(!ui.Join(3000)) throw new Exception("window thread did not stop");
        Console.WriteLine("subclass=removed; window_thread=stopped");
    }
    protected override void WndProc(ref Message m) {
        if(m.Msg==0x0010) {Application.ExitThread(); return;}
        base.WndProc(ref m);
    }
}
'@
[TimeoutProbe]::Run()
