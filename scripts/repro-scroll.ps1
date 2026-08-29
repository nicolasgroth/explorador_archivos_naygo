# Naygo — reproducción interactiva de selección y scroll (smoke de UI).
# Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
#
# Lanza una instancia aislada, simula entrada Windows y deja capturas para revisión.
[CmdletBinding()]
param(
    [string]$Exe = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\naygo.exe'),
    [string]$Shots = (Join-Path (Split-Path -Parent $PSScriptRoot) 'target\ui-smoke\scroll')
)

$ErrorActionPreference = "Stop"
if (-not (Test-Path -LiteralPath $Exe)) { throw "No existe Naygo: $Exe" }
New-Item -ItemType Directory -Path $Shots -Force | Out-Null
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
    if (-not (Test-Path -LiteralPath "$shots\$name.png")) { throw "No se creó la captura $name" }
    Write-Host "shot: $name"
}
$proc = Start-Process -FilePath $Exe -PassThru
for ($i = 0; $i -lt 40; $i++) { Start-Sleep -Milliseconds 500; $proc.Refresh(); if ($proc.MainWindowHandle -ne [IntPtr]::Zero) { break } }
Start-Sleep -Seconds 3
if ($proc.MainWindowHandle -eq [IntPtr]::Zero) { throw 'Naygo no abrió una ventana interactiva.' }
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
