# Naygo — build de distribución supervisado, sin alterar el perfil ni ajustes del equipo.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT

[CmdletBinding()]
param(
    [ValidateRange(0.125,24)][double]$MemoryGiB = 12,
    [ValidateRange(1,32)][double]$MinAvailableGiB = 8,
    [switch]$Probe,
    [switch]$Verify,
    [switch]$W07ReleaseMeasurement,
    [switch]$W07InputMeasurement,
    [switch]$W07MeshPreviewMeasurement,
    # Modo interno: espera permiso antes de lanzar cualquier compilador.
    [switch]$Child,
    [string]$GateName
)
$ErrorActionPreference = 'Stop'
if (
    ($Probe -and $Verify) -or ($Probe -and $W07ReleaseMeasurement) -or ($Probe -and $W07InputMeasurement) -or
    ($Probe -and $W07MeshPreviewMeasurement) -or
    ($Verify -and $W07ReleaseMeasurement) -or ($Verify -and $W07InputMeasurement) -or
    ($Verify -and $W07MeshPreviewMeasurement) -or
    ($W07ReleaseMeasurement -and $W07InputMeasurement) -or
    ($W07ReleaseMeasurement -and $W07MeshPreviewMeasurement) -or
    ($W07InputMeasurement -and $W07MeshPreviewMeasurement)
) {
    throw 'Probe, Verify y modos de medición W-07 son excluyentes.'
}
if ($Child) {
    $childGate = [Threading.EventWaitHandle]::OpenExisting($GateName)
    try {
        if (-not $childGate.WaitOne(30000)) { throw 'No se recibió permiso del supervisor.' }
    } finally { $childGate.Dispose() }
    if ([Diagnostics.Process]::GetCurrentProcess().PriorityClass -ne 'BelowNormal') {
        throw 'Prioridad del Job no aplicada.'
    }
    if ($Probe) {
        # Test acotado: con -MemoryGiB 0.25, esta reserva debe fallar antes de asignarse.
        try { $probeBytes = [byte[]]::new(512MB) }
        catch {
            if ($_.Exception.GetBaseException() -is [OutOfMemoryException]) {
                Write-Output 'PROBE_OK: Job de memoria y prioridad aplicados.'
                exit 0
            }
            throw
        }
        throw 'La reserva de prueba no fue rechazada por el Job.'
    }
    $env:CARGO_BUILD_JOBS = '1'
    if ($Verify) {
        cargo test --workspace --locked --offline
        if ($LASTEXITCODE -ne 0) { throw 'Falló la suite.' }
        cargo clippy --workspace --all-targets --locked --offline -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Falló el análisis.' }
        exit 0
    }
    if ($W07ReleaseMeasurement) {
        $env:NAYGO_PERF_METRICS = '1'
        cargo test -p naygo-ui-slint --bin naygo --release --locked --offline w10c_headless_sync_measurement -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Falló la medición W-07/W-10C en perfil release.' }
        exit 0
    }
    if ($W07InputMeasurement) {
        $env:NAYGO_PERF_METRICS = '1'
        cargo test -p naygo-ui-slint --bin naygo --release --locked --offline w07_headless_keyboard_callback_measurement -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Falló la medición de eventos de teclado W-07 en perfil release.' }
        exit 0
    }
    if ($W07MeshPreviewMeasurement) {
        cargo bench -p naygo-core --bench mesh_preview --locked --offline
        if ($LASTEXITCODE -ne 0) { throw 'Falló la medición del rasterizador 3D W-07.' }
        exit 0
    }
    & (Join-Path $PSScriptRoot 'build-release.ps1')
    exit 0
}

Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public sealed class NaygoBuildGuard : IDisposable {
    [StructLayout(LayoutKind.Sequential)] struct Basic {
        public long ProcessTime, JobTime;
        public uint Flags;
        public UIntPtr MinWorking, MaxWorking;
        public uint ActiveProcesses;
        public UIntPtr Affinity;
        public uint Priority, Scheduling;
    }
    [StructLayout(LayoutKind.Sequential)] struct Io {
        public ulong ReadOps, WriteOps, OtherOps, ReadBytes, WriteBytes, OtherBytes;
    }
    [StructLayout(LayoutKind.Sequential)] struct Extended {
        public Basic Basic;
        public Io Io;
        public UIntPtr ProcessLimit, JobLimit, PeakProcess, PeakJob;
    }
    [StructLayout(LayoutKind.Sequential)] struct Memory {
        public uint Length, Load;
        public ulong TotalPhysical, AvailablePhysical, TotalCommit, AvailableCommit;
        public ulong TotalVirtual, AvailableVirtual, AvailableExtended;
    }
    [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr CreateJobObject(IntPtr attributes, string name);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool SetInformationJobObject(IntPtr job, int type, ref Extended info, uint size);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool QueryInformationJobObject(IntPtr job, int type, out Extended info, uint size, IntPtr returned);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool AssignProcessToJobObject(IntPtr job, IntPtr process);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool GlobalMemoryStatusEx(ref Memory info);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
    IntPtr job;
    public NaygoBuildGuard(ulong bytes) {
        job = CreateJobObject(IntPtr.Zero, null);
        if (job == IntPtr.Zero) throw new Win32Exception();
        var info = new Extended();
        // KILL_ON_JOB_CLOSE | JOB_MEMORY | PRIORITY_CLASS. Applies to descendants too.
        info.Basic.Flags = 0x2000 | 0x200 | 0x20;
        info.Basic.Priority = 0x4000; // BELOW_NORMAL_PRIORITY_CLASS
        info.JobLimit = new UIntPtr(bytes);
        if (!SetInformationJobObject(job, 9, ref info, (uint)Marshal.SizeOf(info))) {
            var error = new Win32Exception(); Dispose(); throw error;
        }
        Extended applied;
        if (!QueryInformationJobObject(job, 9, out applied, (uint)Marshal.SizeOf(info), IntPtr.Zero)
            || applied.JobLimit.ToUInt64() != bytes || (applied.Basic.Flags & 0x2220) != 0x2220) {
            Dispose(); throw new InvalidOperationException("Build Job limits not applied");
        }
    }
    public void Attach(IntPtr process) {
        if (!AssignProcessToJobObject(job, process)) throw new Win32Exception();
    }
    public ulong Peak() {
        Extended info;
        if (!QueryInformationJobObject(job, 9, out info, (uint)Marshal.SizeOf(typeof(Extended)), IntPtr.Zero)) throw new Win32Exception();
        return info.PeakJob.ToUInt64();
    }
    public static ulong Available() {
        var info = new Memory(); info.Length = (uint)Marshal.SizeOf(info);
        if (!GlobalMemoryStatusEx(ref info)) throw new Win32Exception();
        return Math.Min(info.AvailablePhysical, info.AvailableCommit);
    }
    public void Dispose() {
        if (job != IntPtr.Zero) { CloseHandle(job); job = IntPtr.Zero; }
    }
}
'@

$buildRoot = Split-Path -Parent $PSScriptRoot
$buildLogDir = Join-Path $buildRoot 'target/agent-out'
New-Item -ItemType Directory -Path $buildLogDir -Force | Out-Null
$buildRunId = Get-Date -Format 'yyyyMMdd-HHmmss-fff'
$buildStdout = Join-Path $buildLogDir "guarded-$buildRunId.out.log"
$buildStderr = Join-Path $buildLogDir "guarded-$buildRunId.err.log"
$buildGateName = 'Local\NaygoBuild-' + [guid]::NewGuid().ToString('N')
$buildGate = [Threading.EventWaitHandle]::new($false, [Threading.EventResetMode]::ManualReset, $buildGateName)
$buildGuard = $null
$buildChild = $null
try {
    if ([NaygoBuildGuard]::Available() -lt $MinAvailableGiB * 1GB) {
        throw 'No hay margen de memoria suficiente para iniciar el build.'
    }
    if (-not $Probe) {
        if (Get-Process cargo,rustc,ISCC -ErrorAction SilentlyContinue) {
            throw 'Otra compilación/empaquetado está activo; no ejecutar en paralelo.'
        }
        if (-not $Verify -and -not $W07ReleaseMeasurement -and -not $W07InputMeasurement -and -not $W07MeshPreviewMeasurement) {
            $buildDistPath = Join-Path $buildRoot 'dist'
            if (-not (Test-Path -LiteralPath $buildDistPath)) { New-Item -ItemType Directory -Path $buildDistPath | Out-Null }
            $buildDist = Get-Item -LiteralPath $buildDistPath
            if ($buildDist.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'dist es un enlace.' }
            $buildStage = [IO.Path]::GetFullPath((Join-Path $buildDist.FullName 'portable-stage'))
            if (-not $buildStage.StartsWith($buildDist.FullName + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Stage fuera de dist.' }
            if (Test-Path -LiteralPath $buildStage) { throw 'Stage preexistente: inspeccionarlo antes de borrar.' }
        }
    }
    $buildGuard = [NaygoBuildGuard]::new([uint64]($MemoryGiB * 1GB))
    $buildArguments = '-NoProfile -ExecutionPolicy Bypass -File "' + $PSCommandPath + '" -Child -GateName "' + $buildGateName + '"'
    if ($Probe) { $buildArguments += ' -Probe' }
    if ($Verify) { $buildArguments += ' -Verify' }
    if ($W07ReleaseMeasurement) { $buildArguments += ' -W07ReleaseMeasurement' }
    if ($W07InputMeasurement) { $buildArguments += ' -W07InputMeasurement' }
    if ($W07MeshPreviewMeasurement) { $buildArguments += ' -W07MeshPreviewMeasurement' }
    # El hijo es Windows PowerShell 5.1; no heredar módulos de PowerShell 7 de runtimes externos.
    # Sus ExtendedTypeData puede duplicar la metadata nativa de Microsoft.PowerShell.Security.
    $originalPsModulePath = $env:PSModulePath
    try {
        $env:PSModulePath = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\Modules'
        $buildChild = Start-Process powershell.exe -ArgumentList $buildArguments -WorkingDirectory $buildRoot -WindowStyle Hidden -PassThru -RedirectStandardOutput $buildStdout -RedirectStandardError $buildStderr
    } finally {
        $env:PSModulePath = $originalPsModulePath
    }
    # El hijo está esperando: sin ventana ni compiladores antes de aplicar los límites.
    $buildGuard.Attach($buildChild.Handle)
    $buildGate.Set() | Out-Null
    Write-Output "Build supervisado: limite=$MemoryGiB GiB; margen minimo=$MinAvailableGiB GiB; prioridad baja."
    Write-Output "stdout: $buildStdout"
    Write-Output "stderr: $buildStderr"
    $buildLastReport = [DateTime]::UtcNow
    while (-not $buildChild.WaitForExit(2000)) {
        $buildAvailable = [NaygoBuildGuard]::Available() / 1GB
        if ($buildAvailable -lt $MinAvailableGiB) {
            throw "Build detenido preventivamente: margen disponible $buildAvailable GiB."
        }
        if (([DateTime]::UtcNow - $buildLastReport).TotalSeconds -ge 30) {
            Write-Output ('Pico contable Job: {0:N2} GiB; margen equipo: {1:N2} GiB' -f ($buildGuard.Peak() / 1GB), $buildAvailable)
            $buildLastReport = [DateTime]::UtcNow
        }
    }
    $buildChild.WaitForExit()
    Write-Output ('Pico contable final del Job: {0:N2} GiB' -f ($buildGuard.Peak() / 1GB))
    Get-Content -LiteralPath $buildStdout -Tail 12
    Get-Content -LiteralPath $buildStderr -Tail 12
    if ($buildChild.ExitCode -ne 0) { throw "Build interrumpido/fallido: codigo $($buildChild.ExitCode). Revisar logs." }
} finally {
    # Sólo termina nuestro árbol de compilación. Nunca toca otras apps ni configura el SO.
    if ($null -ne $buildGuard) { $buildGuard.Dispose() }
    if ($null -ne $buildChild) {
        if (-not $buildChild.HasExited) { $buildChild.Kill() }
        $buildChild.WaitForExit()
        $buildChild.Dispose()
    }
    $buildGate.Dispose()
}
