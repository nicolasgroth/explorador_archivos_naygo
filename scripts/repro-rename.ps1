# Driver de reproducción (temporal): lanza Naygo debug, lo despierta, hace CLIC real en la
# primera fila del panel (foco legítimo + selección), F2, teclea y fotografía.
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
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, int e);
    public struct RECT { public int Left, Top, Right, Bottom; }
    public static void Click(int x, int y) {
        SetCursorPos(x, y);
        mouse_event(0x0002, 0, 0, 0, 0); // LEFTDOWN
        mouse_event(0x0004, 0, 0, 0, 0); // LEFTUP
    }
}
"@

function Shot($name) {
    $b = New-Object System.Drawing.Bitmap([System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Width,
                                          [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Height)
    $g = [System.Drawing.Graphics]::FromImage($b)
    $g.CopyFromScreen(0, 0, 0, 0, $b.Size)
    $b.Save("$shots\$name.png", [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $b.Dispose()
    Write-Host "shot: $name"
}

$proc = Start-Process -FilePath $exe -PassThru
Write-Host "PID: $($proc.Id)"
for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep -Milliseconds 500
    $proc.Refresh()
    if ($proc.MainWindowHandle -ne [IntPtr]::Zero) { break }
}
Start-Sleep -Seconds 3
[System.Windows.Forms.SendKeys]::SendWait("^%z")
Start-Sleep -Seconds 2
$proc.Refresh()
[FG]::ShowWindow($proc.MainWindowHandle, 9) | Out-Null
Start-Sleep -Milliseconds 800
# Mover la ventana a una zona LIBRE de la pantalla (posición conocida para los clics).
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class MV { [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int ht, bool r); }
"@
[MV]::MoveWindow($proc.MainWindowHandle, 900, 60, 1400, 900, $true) | Out-Null
Start-Sleep -Milliseconds 800
[FG]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 500

# Geometría real de la ventana y CLIC dentro del primer panel de archivos (foco legítimo).
$r = New-Object FG+RECT
[FG]::GetWindowRect($proc.MainWindowHandle, [ref]$r) | Out-Null
Write-Host "rect: $($r.Left),$($r.Top) $($r.Right)x$($r.Bottom)"
$cx = 900 + 190 + 160   # árbol (~190px) + medio del panel rename-fixture
$cy = 60 + 253             # fila 3 (y.txt): barras ~198px + 2.5 filas
[FG]::SetCursorPos($cx, $cy) | Out-Null
Start-Sleep -Milliseconds 400
[FG]::Click($cx, $cy)
Start-Sleep -Milliseconds 800
Shot "repro-1-click"

# Editor de RUTA (Ctrl+L): ¿Shift+x inserta ahí?
[System.Windows.Forms.SendKeys]::SendWait("^l")
Start-Sleep -Seconds 4
Shot "repro-2-path-editor"
[System.Windows.Forms.SendKeys]::SendWait("abc")
Start-Sleep -Seconds 2
[System.Windows.Forms.SendKeys]::SendWait("{LEFT}{LEFT}")
Start-Sleep -Seconds 1
# Shift+x: el keydown de Shift burbujea → activate → sync (antes re-montaba el editor).
[System.Windows.Forms.SendKeys]::SendWait("+x")
Start-Sleep -Seconds 1
# Si el editor NO se re-montó, z entra a mitad ("abzcx"); si se re-montó, cae al final.
[System.Windows.Forms.SendKeys]::SendWait("z")
Start-Sleep -Seconds 2
Shot "repro-4-tras-teclas"

[System.Windows.Forms.SendKeys]::SendWait("{ESC}")
Start-Sleep -Milliseconds 400
Write-Host "fin; cerrando"
Stop-Process -Id $proc.Id -Force
