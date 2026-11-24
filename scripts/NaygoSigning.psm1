# Naygo — validación de firmas Authenticode para artefactos SignPath.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT

Set-StrictMode -Version Latest

function Assert-NaygoSignPathProductMetadataValues {
    [CmdletBinding()]
    param(
        [AllowNull()]
        [AllowEmptyString()]
        [string] $ProductNameValue,
        [AllowNull()]
        [AllowEmptyString()]
        [string] $ProductVersionValue,
        [Parameter(Mandatory)]
        [string] $ExpectedProductName,
        [Parameter(Mandatory)]
        [string] $ExpectedProductVersion,
        [Parameter(Mandatory)]
        [string] $ArtifactPath
    )

    # Inno Setup 6.7.3 stores these fixed-width PE strings with trailing U+0020 padding.
    # Keep leading/internal whitespace significant; SignPath restrictions still require
    # the actual product name and version, and its parser behavior must be confirmed remotely.
    $actualProductName = if ($null -eq $ProductNameValue) {
        ''
    } else {
        $ProductNameValue.TrimEnd([char]32)
    }
    $actualProductVersion = if ($null -eq $ProductVersionValue) {
        ''
    } else {
        $ProductVersionValue.TrimEnd([char]32)
    }

    if ($actualProductName -cne $ExpectedProductName) {
        throw "ProductName inválido en $ArtifactPath (esperado '$ExpectedProductName', observado '$actualProductName')."
    }
    if ($actualProductVersion -cne $ExpectedProductVersion) {
        throw "ProductVersion inválido en $ArtifactPath (esperado '$ExpectedProductVersion', observado '$actualProductVersion')."
    }
}

function Assert-NaygoSignPathProductMetadata {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string] $Path,
        [Parameter(Mandatory)]
        [string] $ExpectedProductName,
        [Parameter(Mandatory)]
        [string] $ExpectedProductVersion
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "No existe el artefacto para validar metadatos: $Path"
    }

    $versionInfo = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($Path)
    Assert-NaygoSignPathProductMetadataValues `
        -ProductNameValue $versionInfo.ProductName `
        -ProductVersionValue $versionInfo.ProductVersion `
        -ExpectedProductName $ExpectedProductName `
        -ExpectedProductVersion $ExpectedProductVersion `
        -ArtifactPath $Path
}

function Assert-NaygoSignPathPublisher {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [System.Security.Cryptography.X509Certificates.X509Certificate2] $Certificate,
        [Parameter(Mandatory)]
        [string] $ArtifactPath
    )

    $publisher = $Certificate.GetNameInfo(
        [System.Security.Cryptography.X509Certificates.X509NameType]::SimpleName,
        $false
    )
    Write-Host "Firmante: $publisher"
    Write-Host "Sujeto: $($Certificate.Subject)"
    Write-Host "Emisor: $($Certificate.Issuer)"
    Write-Host "Thumbprint: $($Certificate.Thumbprint)"

    if ($publisher -cne 'SignPath Foundation') {
        throw "El firmante de $ArtifactPath no es SignPath Foundation (SimpleName: '$publisher')."
    }
}

function Assert-NaygoSignPathSignature {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string] $Path
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "No existe el artefacto firmado: $Path"
    }

    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne [System.Management.Automation.SignatureStatus]::Valid) {
        throw "Firma Authenticode inválida en $Path ($($signature.Status))."
    }
    if ($null -eq $signature.SignerCertificate) {
        throw "La firma Authenticode de $Path no contiene certificado de firmante."
    }

    Assert-NaygoSignPathPublisher -Certificate $signature.SignerCertificate -ArtifactPath $Path
}

Export-ModuleMember -Function `
    Assert-NaygoSignPathProductMetadata, `
    Assert-NaygoSignPathProductMetadataValues, `
    Assert-NaygoSignPathPublisher, `
    Assert-NaygoSignPathSignature
