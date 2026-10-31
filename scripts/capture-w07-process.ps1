# Naygo — captura pasiva de recursos de proceso para W-07.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT

[CmdletBinding()]
param(
    [int] $ProcessId = 0,
    [string] $ProcessName = 'naygo',
    [ValidateRange(1, 20)]
    [int] $WindowCount = 3,
    [ValidateRange(1, 3600)]
    [int] $WindowSeconds = 30,
    [ValidateRange(1, 30)]
    [int] $SampleIntervalSeconds = 2,
    [string] $OutputPath
)

$ErrorActionPreference = 'Stop'

function Get-ProcessSnapshot {
    param([Parameter(Mandatory)] [int] $RootProcessId)

    $systemProcesses = @(Get-CimInstance -ClassName Win32_Process -Property ProcessId, ParentProcessId, Name)
    $selectedIds = [System.Collections.Generic.HashSet[int]]::new()
    $queue = [System.Collections.Generic.Queue[int]]::new()
    $null = $selectedIds.Add($RootProcessId)
    $queue.Enqueue($RootProcessId)

    while ($queue.Count -gt 0) {
        $parentId = $queue.Dequeue()
        foreach ($candidate in $systemProcesses) {
            if ([int]$candidate.ParentProcessId -eq $parentId) {
                $childId = [int]$candidate.ProcessId
                if ($selectedIds.Add($childId)) {
                    $queue.Enqueue($childId)
                }
            }
        }
    }

    $members = [System.Collections.Generic.List[object]]::new()
    foreach ($candidateId in $selectedIds) {
        $observedProcess = $systemProcesses |
            Where-Object { [int]$_.ProcessId -eq $candidateId } |
            Select-Object -First 1
        $process = Get-Process -Id $candidateId -ErrorAction SilentlyContinue
        if ($null -eq $process) {
            continue
        }

        $startedAt = $null
        try { $startedAt = $process.StartTime.ToString('o') } catch {}
        $cpuSeconds = 0.0
        try { $cpuSeconds = [double]$process.CPU } catch {}
        $members.Add([pscustomobject]@{
            process_id = $candidateId
            parent_process_id = if ($null -ne $observedProcess) {
                [int]$observedProcess.ParentProcessId
            } else { $null }
            process_name = $process.ProcessName
            started_at = $startedAt
            working_set_mib = [math]::Round($process.WorkingSet64 / 1MB, 1)
            private_mib = [math]::Round($process.PrivateMemorySize64 / 1MB, 1)
            handles = $process.HandleCount
            threads = $process.Threads.Count
            cpu_seconds = [math]::Round($cpuSeconds, 3)
        })
    }

    $root = $members | Where-Object { $_.process_id -eq $RootProcessId } | Select-Object -First 1
    if ($null -eq $root) {
        throw "El proceso raíz $RootProcessId terminó o no se pudo consultar durante la captura."
    }

    [pscustomobject]@{
        time = (Get-Date).ToString('o')
        process_count = $members.Count
        working_set_mib = $root.working_set_mib
        private_mib = $root.private_mib
        handles = $root.handles
        threads = $root.threads
        cpu_seconds = $root.cpu_seconds
        tree_working_set_mib_sum = [math]::Round(
            (($members | Measure-Object -Property working_set_mib -Sum).Sum), 1
        )
        tree_private_mib_sum = [math]::Round(
            (($members | Measure-Object -Property private_mib -Sum).Sum), 1
        )
        tree_handles = ($members | Measure-Object -Property handles -Sum).Sum
        tree_threads = ($members | Measure-Object -Property threads -Sum).Sum
        tree_cpu_seconds = [math]::Round(
            (($members | Measure-Object -Property cpu_seconds -Sum).Sum), 3
        )
        processes = @($members.ToArray())
    }
}

function Get-TreeCpuDelta {
    param(
        [Parameter(Mandatory)] [object] $Previous,
        [Parameter(Mandatory)] [object] $Current
    )

    $previousCpuByProcess = @{}
    foreach ($member in @($Previous.processes)) {
        $key = "$($member.process_id)|$($member.started_at)"
        $previousCpuByProcess[$key] = [double]$member.cpu_seconds
    }

    $deltaSeconds = 0.0
    foreach ($member in @($Current.processes)) {
        $key = "$($member.process_id)|$($member.started_at)"
        if ($previousCpuByProcess.ContainsKey($key)) {
            $delta = [double]$member.cpu_seconds - $previousCpuByProcess[$key]
            if ($delta -gt 0) { $deltaSeconds += $delta }
        }
    }
    return [math]::Round($deltaSeconds, 3)
}

function Get-TreePeak {
    param(
        [Parameter(Mandatory)] [object[]] $Snapshots,
        [Parameter(Mandatory)] [string] $Property
    )

    $peak = 0.0
    foreach ($snapshot in $Snapshots) {
        $value = [double]$snapshot.$Property
        if ($value -gt $peak) { $peak = $value }
    }
    return [math]::Round($peak, 1)
}

if ($ProcessId -gt 0) {
    $process = Get-Process -Id $ProcessId -ErrorAction Stop
} else {
    $matches = @(Get-Process -Name $ProcessName -ErrorAction SilentlyContinue)
    if ($matches.Count -eq 0) {
        throw "No hay un proceso '$ProcessName' en ejecución."
    }
    if ($matches.Count -gt 1) {
        $candidateIds = [System.Collections.Generic.HashSet[int]]::new()
        foreach ($candidate in $matches) {
            $null = $candidateIds.Add([int]$candidate.Id)
        }
        $systemProcesses = @(Get-CimInstance -ClassName Win32_Process -Property ProcessId, ParentProcessId, Name)
        $nestedCandidateIds = [System.Collections.Generic.HashSet[int]]::new()
        foreach ($record in $systemProcesses) {
            $parentIsCandidate = $candidateIds.Contains([int]$record.ParentProcessId)
            $processIsCandidate = $candidateIds.Contains([int]$record.ProcessId)
            if ($parentIsCandidate -and $processIsCandidate) {
                $null = $nestedCandidateIds.Add([int]$record.ProcessId)
            }
        }
        $roots = @($matches | Where-Object {
            -not $nestedCandidateIds.Contains([int]$_.Id)
        })
        if ($roots.Count -ne 1) {
            $ids = ($roots | ForEach-Object { $_.Id }) -join ', '
            throw "Hay varios procesos raíz '$ProcessName' (PID: $ids); indica -ProcessId."
        }
        $process = $roots[0]
    } else {
        $process = $matches[0]
    }
}

$processIdCaptured = $process.Id
$startedAt = $process.StartTime.ToString('o')
$samples = [System.Collections.Generic.List[object]]::new()

for ($index = 1; $index -le $WindowCount; $index++) {
    $process = Get-Process -Id $processIdCaptured -ErrorAction Stop
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $windowSnapshots = [System.Collections.Generic.List[object]]::new()
    $windowSnapshots.Add((Get-ProcessSnapshot -RootProcessId $processIdCaptured))
    $nextSampleAt = [math]::Min([double]$SampleIntervalSeconds, [double]$WindowSeconds)
    while ($timer.Elapsed.TotalSeconds -lt $WindowSeconds) {
        $remaining = $nextSampleAt - $timer.Elapsed.TotalSeconds
        if ($remaining -gt 0) {
            Start-Sleep -Milliseconds ([math]::Max(1, [int][math]::Ceiling($remaining * 1000)))
        }
        $windowSnapshots.Add((Get-ProcessSnapshot -RootProcessId $processIdCaptured))
        $nextSampleAt = [math]::Min(
            $nextSampleAt + $SampleIntervalSeconds,
            [double]$WindowSeconds
        )
    }
    $timer.Stop()
    $before = $windowSnapshots[0]
    $after = $windowSnapshots[$windowSnapshots.Count - 1]
    $treeCpuDelta = 0.0
    for ($snapshotIndex = 1; $snapshotIndex -lt $windowSnapshots.Count; $snapshotIndex++) {
        $treeCpuDelta += Get-TreeCpuDelta `
            -Previous $windowSnapshots[$snapshotIndex - 1] `
            -Current $windowSnapshots[$snapshotIndex]
    }
    $observedProcesses = [System.Collections.Generic.Dictionary[string, object]]::new()
    foreach ($snapshot in $windowSnapshots) {
        foreach ($member in @($snapshot.processes)) {
            $key = "$($member.process_id)|$($member.started_at)"
            if (-not $observedProcesses.ContainsKey($key)) {
                $observedProcesses.Add($key, [pscustomobject]@{
                    process_id = $member.process_id
                    parent_process_id = $member.parent_process_id
                    process_name = $member.process_name
                    started_at = $member.started_at
                })
            }
        }
    }

    $samples.Add([pscustomobject]@{
        window = $index
        seconds = $WindowSeconds
        sample_interval_seconds = $SampleIntervalSeconds
        observation_count = $windowSnapshots.Count
        observed_processes = @($observedProcesses.Values)
        peak_tree_working_set_mib_sum = Get-TreePeak -Snapshots @($windowSnapshots) -Property tree_working_set_mib_sum
        peak_tree_private_mib_sum = Get-TreePeak -Snapshots @($windowSnapshots) -Property tree_private_mib_sum
        peak_tree_handles = Get-TreePeak -Snapshots @($windowSnapshots) -Property tree_handles
        peak_tree_threads = Get-TreePeak -Snapshots @($windowSnapshots) -Property tree_threads
        before = $before
        after = $after
        cpu_delta_seconds = [math]::Round($after.cpu_seconds - $before.cpu_seconds, 3)
        tree_cpu_delta_seconds = [math]::Round($treeCpuDelta, 3)
    })
}

$smbMappings = $null
$smbMappingError = $null
if (Get-Command Get-SmbMapping -ErrorAction SilentlyContinue) {
    try {
        $smbMappings = @(Get-SmbMapping | Select-Object LocalPath, RemotePath, Status)
    } catch {
        $smbMappingError = $_.Exception.Message
    }
}

if (-not $OutputPath) {
    $timestamp = Get-Date -Format 'yyyyMMdd-HHmmss'
    $outputDirectory = Join-Path $PSScriptRoot '..\target\agent-out'
    $OutputPath = Join-Path $outputDirectory "w07-process-$timestamp.json"
}

$resolvedOutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$parentDirectory = Split-Path -Parent $resolvedOutputPath
if (-not (Test-Path -LiteralPath $parentDirectory -PathType Container)) {
    New-Item -ItemType Directory -Path $parentDirectory -Force | Out-Null
}

$record = [pscustomobject]@{
    schema = 'naygo.w07.process-sample.v3'
    captured_at = (Get-Date).ToString('o')
    process_name = $process.ProcessName
    process_id = $processIdCaptured
    process_started_at = $startedAt
    windows = @($samples.ToArray())
    smb_mappings = $smbMappings
    smb_mapping_error = $smbMappingError
    notes = 'Passive process observation only. Each window samples the root and descendants at the configured interval; processes shorter than the interval may still be missed. Peak tree working-set values are sums and can double-count shared pages. Tree CPU delta sums positive deltas for matching process identities in adjacent samples, so CPU used before a process first appears or after its last sample is not counted. Sampling adds modest observation overhead. This script does not interact with Naygo or confirm application inactivity and does not measure UI latency or file-system performance. Enable NAYGO_PERF_METRICS=1 before starting Naygo to capture internal timing metrics separately.'
}

$record | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $resolvedOutputPath -Encoding UTF8
Write-Output $resolvedOutputPath
