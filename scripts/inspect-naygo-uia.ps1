# Naygo — inspeccion no interactiva del arbol UI Automation para W-13.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT

[CmdletBinding()]
param(
    [int] $ProcessId = 0,
    [ValidateRange(1, 20)]
    [int] $MaxDepth = 8,
    [ValidateRange(20, 5000)]
    [int] $MaxElements = 1200
)

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

if ($ProcessId -gt 0) {
    $process = Get-Process -Id $ProcessId -ErrorAction Stop
} else {
    $process = Get-Process -Name 'naygo' -ErrorAction Stop |
        Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } |
        Select-Object -First 1
}

if ($null -eq $process -or $process.MainWindowHandle -eq [IntPtr]::Zero) {
    throw 'No se encontro una ventana principal de Naygo para inspeccionar.'
}

$root = [System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
$walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
$pending = [System.Collections.Generic.Queue[object]]::new()
$pending.Enqueue([pscustomobject]@{ Element = $root; Depth = 0 })
$counts = [System.Collections.Generic.Dictionary[string, int]]::new()
$unnamedFocusableTypes = [System.Collections.Generic.Dictionary[string, int]]::new()
$patternCounts = [System.Collections.Generic.Dictionary[string, int]]::new()
$focusablePatternCounts = [System.Collections.Generic.Dictionary[string, int]]::new()
$buttonsWithoutInvokeDetails = [System.Collections.Generic.Dictionary[string, int]]::new()
$patterns = @(
    [pscustomobject]@{ Name = 'Invoke'; Pattern = [System.Windows.Automation.InvokePattern]::Pattern },
    [pscustomobject]@{ Name = 'Selection'; Pattern = [System.Windows.Automation.SelectionPattern]::Pattern },
    [pscustomobject]@{ Name = 'SelectionItem'; Pattern = [System.Windows.Automation.SelectionItemPattern]::Pattern },
    [pscustomobject]@{ Name = 'ExpandCollapse'; Pattern = [System.Windows.Automation.ExpandCollapsePattern]::Pattern },
    [pscustomobject]@{ Name = 'Toggle'; Pattern = [System.Windows.Automation.TogglePattern]::Pattern },
    [pscustomobject]@{ Name = 'Value'; Pattern = [System.Windows.Automation.ValuePattern]::Pattern },
    [pscustomobject]@{ Name = 'RangeValue'; Pattern = [System.Windows.Automation.RangeValuePattern]::Pattern },
    [pscustomobject]@{ Name = 'ScrollItem'; Pattern = [System.Windows.Automation.ScrollItemPattern]::Pattern },
    [pscustomobject]@{ Name = 'Text'; Pattern = [System.Windows.Automation.TextPattern]::Pattern }
)
$descendants = 0
$named = 0
$keyboardFocusable = 0
$namedFocusable = 0
$buttonsWithoutInvoke = 0
$focusableButtonsWithoutInvoke = 0
$truncated = $false

while ($pending.Count -gt 0) {
    if ($descendants -ge $MaxElements) {
        $truncated = $true
        break
    }

    $item = $pending.Dequeue()
    if ($item.Depth -ge $MaxDepth) {
        continue
    }

    try {
        $child = $walker.GetFirstChild($item.Element)
        while ($null -ne $child) {
            if ($descendants -ge $MaxElements) {
                $truncated = $true
                break
            }

            try {
                $current = $child.Current
                $controlType = $current.ControlType.ProgrammaticName
                if (-not $counts.ContainsKey($controlType)) {
                    $counts.Add($controlType, 0)
                }
                $counts[$controlType]++
                $descendants++

                $hasName = -not [string]::IsNullOrWhiteSpace($current.Name)
                if ($hasName) { $named++ }
                if ($current.IsKeyboardFocusable) {
                    $keyboardFocusable++
                    if ($hasName) {
                        $namedFocusable++
                    } else {
                        if (-not $unnamedFocusableTypes.ContainsKey($controlType)) {
                            $unnamedFocusableTypes.Add($controlType, 0)
                        }
                        $unnamedFocusableTypes[$controlType]++
                    }
                }

                # Consultar patrones no los ejecuta: sólo informa qué acciones/valores
                # anuncia el proveedor UIA. No leer Value, texto ni otros datos del control.
                $supportsInvoke = $false
                foreach ($patternInfo in $patterns) {
                    try {
                        $null = $child.GetCurrentPattern($patternInfo.Pattern)
                        if ($patternInfo.Name -eq 'Invoke') { $supportsInvoke = $true }
                        if (-not $patternCounts.ContainsKey($patternInfo.Name)) {
                            $patternCounts.Add($patternInfo.Name, 0)
                        }
                        $patternCounts[$patternInfo.Name]++
                        if ($current.IsKeyboardFocusable) {
                            if (-not $focusablePatternCounts.ContainsKey($patternInfo.Name)) {
                                $focusablePatternCounts.Add($patternInfo.Name, 0)
                            }
                            $focusablePatternCounts[$patternInfo.Name]++
                        }
                    } catch {
                        # El patrón no está disponible o el elemento desapareció.
                    }
                }
                if ($controlType -eq 'ControlType.Button' -and -not $supportsInvoke) {
                    $buttonsWithoutInvoke++
                    if ($current.IsKeyboardFocusable) { $focusableButtonsWithoutInvoke++ }
                    # No registrar Name: puede contener un nombre de archivo. La clase, AutomationId
                    # y presencia de nombre bastan para vincular el control con el código UI.
                    $detailKey = 'class=' + $current.ClassName +
                        '|automation_id=' + $current.AutomationId +
                        '|focusable=' + $current.IsKeyboardFocusable +
                        '|name_empty=' + [string]::IsNullOrWhiteSpace($current.Name)
                    if (-not $buttonsWithoutInvokeDetails.ContainsKey($detailKey)) {
                        $buttonsWithoutInvokeDetails.Add($detailKey, 0)
                    }
                    $buttonsWithoutInvokeDetails[$detailKey]++
                }

                if ($item.Depth + 1 -lt $MaxDepth) {
                    $pending.Enqueue([pscustomobject]@{
                        Element = $child
                        Depth = $item.Depth + 1
                    })
                }
            } catch {
                # Un elemento que desaparece durante la inspección no invalida el resto del árbol.
            }

            try { $child = $walker.GetNextSibling($child) } catch { break }
        }
    } catch {
        # Un provider que falla en una rama no debe ocultar las demás.
    }
}

if ($descendants -eq 0) {
    throw 'UI Automation no expone controles descendientes de la ventana principal de Naygo.'
}

[pscustomobject]@{
    ProcessId = $process.Id
    WindowControlType = $root.Current.ControlType.ProgrammaticName
    Descendants = $descendants
    NamedElements = $named
    KeyboardFocusableElements = $keyboardFocusable
    NamedKeyboardFocusableElements = $namedFocusable
    UnnamedKeyboardFocusableTypes = ($unnamedFocusableTypes.GetEnumerator() | Sort-Object Key | ForEach-Object {
        "$($_.Key)=$($_.Value)"
    }) -join '; '
    SupportedPatterns = ($patternCounts.GetEnumerator() | Sort-Object Key | ForEach-Object {
        "$($_.Key)=$($_.Value)"
    }) -join '; '
    FocusableSupportedPatterns = ($focusablePatternCounts.GetEnumerator() | Sort-Object Key | ForEach-Object {
        "$($_.Key)=$($_.Value)"
    }) -join '; '
    ButtonsWithoutInvoke = $buttonsWithoutInvoke
    FocusableButtonsWithoutInvoke = $focusableButtonsWithoutInvoke
    ButtonsWithoutInvokeDetails = ($buttonsWithoutInvokeDetails.GetEnumerator() | Sort-Object Key | ForEach-Object {
        "$($_.Key)=$($_.Value)"
    }) -join '; '
    Truncated = $truncated
    ControlTypes = ($counts.GetEnumerator() | Sort-Object Key | ForEach-Object {
        "$($_.Key)=$($_.Value)"
    }) -join '; '
    Note = 'Inspeccion de solo lectura; consulta disponibilidad de patrones sin invocarlos y no registra nombres/rutas/contenido. No valida una sesion real de Narrador.'
}
