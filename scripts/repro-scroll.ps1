# Repro scroll: rueda sobre el panel ngrot (204 ítems) y comparar antes/después.
$ErrorActionPreference = "Stop"
$exe = "D:\Empresas\ISGroth\explorador_de_archivos\target\debug\naygo.exe"
$shots = "D:\Empresas\ISGroth\explorador_de_archivos\target\agent-out"
$env:NAYGO_DEBUG_MULTI_INSTANCE = "1"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class FG {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int ht, bool r);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, int d, int e);
    public static void Wheel(int x, int y, int delta) { SetCursorPos(x, y); mouse_event(0x0800, 0, 0, delta, 0); }
    public static void Click(int x, int y) { SetCursorPos(x, y); mouse_event(0x0002, 0, 0, 0, 0); mouse_event(0x0004, 0, 0, 0, 0); }
}
"@
function Shot($name) {
    $b = New-Object System.Drawing.Bitmap([System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Width, [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Height)
    $g = [System.Drawing.Graphics]::FromImage($b)
    $g.CopyFromScreen(0, 0, 0, 0, $b.Size)
    $b.Save("$shots\$name.png", [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $b.Dispose()
    Write-Host "shot: $name"
}
$proc = Start-Process -FilePath $exe -PassThru
for ($i = 0; $i -lt 40; $i++) { Start-Sleep -Milliseconds 500; $proc.Refresh(); if ($proc.MainWindowHandle -ne [IntPtr]::Zero) { break } }
Start-Sleep -Seconds 3
[System.Windows.Forms.SendKeys]::SendWait("^%z")
Start-Sleep -Seconds 2
$proc.Refresh()
[FG]::ShowWindow($proc.MainWindowHandle, 9) | Out-Null
[FG]::MoveWindow($proc.MainWindowHandle, 900, 60, 1400, 900, $true) | Out-Null
Start-Sleep -Milliseconds 800
[FG]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 500
# Clic en el panel ngrot y tipear filtro "k" (condiciones del usuario: filtro activo).
# Clic en una fila VISIBLE del panel ngrot (activa el panel y selecciona).
[FG]::SetCursorPos(1650, 360) | Out-Null
Start-Sleep -Milliseconds 300
[FG]::Click(1650, 360)
Start-Sleep -Milliseconds 700
Shot "scroll-0-tras-click"
# Tipear "k" para buscar: la selección debe saltar al primer match y la vista seguirlo.
[System.Windows.Forms.SendKeys]::SendWait("n8")
Start-Sleep -Seconds 2
Shot "scroll-1-tras-k"
Shot "scroll-2-final"
Write-Host "fin; cerrando"
Stop-Process -Id $proc.Id -Force
