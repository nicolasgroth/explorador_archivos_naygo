# Naygo — mide tiempo y pico de memoria de un comando de compilación
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT
#
# Uso:
#   .\scripts\measure-build.ps1 -Command "cargo build -p naygo-ui-slint --locked" -Label "debug no-op"
#
# Lanza el comando con `cmd /c` (stdout/stderr van a archivos temporales, así PowerShell 5.1
# no convierte el stderr de cargo en errores), muestrea cada 500 ms la suma de WorkingSet y
# de memoria privada de los procesos DESCENDIENTES del comando (árbol por ParentProcessId)
# cuyo nombre sea cargo / rustc / build-script-build* / link / cl / lld-link, y al terminar
# imprime una línea: etiqueta | tiempo | pico WS GiB | pico privada GiB | exit

param(
    [Parameter(Mandatory = $true)][string]$Command,
    [Parameter(Mandatory = $true)][string]$Label,
    [int]$SampleMs = 500
)

$ErrorActionPreference = 'Stop'
$outFile = [System.IO.Path]::GetTempFileName()
$errFile = [System.IO.Path]::GetTempFileName()
$pattern = '^(cargo|rustc|link|lld-link|cl|build-script-build.*|build_script_build.*)\.exe$'

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath 'cmd.exe' -ArgumentList @('/c', $Command) -PassThru -NoNewWindow `
    -RedirectStandardOutput $outFile -RedirectStandardError $errFile
$rootPid = $proc.Id
# Hay que tocar el handle: sin esto, PS 5.1 deja ExitCode en $null tras el WaitForExit.
$null = $proc.Handle

$peakWs = 0.0
$peakPriv = 0.0
while (-not $proc.HasExited) {
    $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name, WorkingSetSize, PrivatePageCount
    $children = @{}
    foreach ($p in $all) {
        if (-not $children.ContainsKey([int]$p.ParentProcessId)) { $children[[int]$p.ParentProcessId] = @() }
        $children[[int]$p.ParentProcessId] += $p
    }
    $byPid = @{}
    foreach ($p in $all) { $byPid[[int]$p.ProcessId] = $p }
    $stack = New-Object System.Collections.Stack
    $stack.Push($rootPid)
    $ws = 0.0
    $priv = 0.0
    while ($stack.Count -gt 0) {
        $cur = [int]$stack.Pop()
        if ($children.ContainsKey($cur)) {
            foreach ($c in $children[$cur]) {
                $stack.Push([int]$c.ProcessId)
                if ($c.Name -match $pattern) {
                    $ws += [double]$c.WorkingSetSize
                    $priv += [double]$c.PrivatePageCount
                }
            }
        }
    }
    if ($ws -gt $peakWs) { $peakWs = $ws }
    if ($priv -gt $peakPriv) { $peakPriv = $priv }
    Start-Sleep -Milliseconds $SampleMs
}
$proc.WaitForExit()
$sw.Stop()
$exit = $proc.ExitCode

$gib = 1024.0 * 1024.0 * 1024.0
$inv = [System.Globalization.CultureInfo]::InvariantCulture
$elapsed = $sw.Elapsed.ToString('hh\:mm\:ss\.f')
$line = '{0} | {1} | {2} GiB WS | {3} GiB private | exit {4}' -f $Label, $elapsed,
    ($peakWs / $gib).ToString('0.00', $inv), ($peakPriv / $gib).ToString('0.00', $inv), $exit
Write-Output $line

if ($null -eq $exit -or $exit -ne 0) {
    Write-Output '--- stderr (últimas 20 líneas) ---'
    Get-Content $errFile -Tail 20
}
Remove-Item $outFile, $errFile -ErrorAction SilentlyContinue
exit $exit
