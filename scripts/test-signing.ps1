# Naygo — pruebas locales de la validación de firma SignPath.
# Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
# SPDX-License-Identifier: MIT

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $PSScriptRoot 'NaygoSigning.psm1') -Force

Assert-NaygoSignPathProductMetadataValues `
    -ProductNameValue ('Naygo' + (' ' * 55)) `
    -ProductVersionValue ('0.5.3' + (' ' * 45)) `
    -ExpectedProductName 'Naygo' `
    -ExpectedProductVersion '0.5.3' `
    -ArtifactPath 'synthetic-inno-setup.exe'

foreach ($badMetadata in @(
    @{ Name = ' Naygo '; Version = '0.5.3'; ExpectedError = 'ProductName' },
    @{ Name = 'Naygo '; Version = '0.5.2'; ExpectedError = 'ProductVersion' }
)) {
    $rejected = $false
    try {
        Assert-NaygoSignPathProductMetadataValues `
            -ProductNameValue $badMetadata.Name `
            -ProductVersionValue $badMetadata.Version `
            -ExpectedProductName 'Naygo' `
            -ExpectedProductVersion '0.5.3' `
            -ArtifactPath 'synthetic-invalid.exe'
    } catch {
        $rejected = $_.Exception.Message -match $badMetadata.ExpectedError
    }
    if (-not $rejected) {
        throw "El helper no rechazó metadatos inválidos: $($badMetadata.Name) / $($badMetadata.Version)."
    }
}

$rsa = [System.Security.Cryptography.RSA]::Create(2048)
$certificates = [System.Collections.Generic.List[System.Security.Cryptography.X509Certificates.X509Certificate2]]::new()
$temporaryFile = $null
try {
    $validSubject = [System.Security.Cryptography.X509Certificates.X500DistinguishedName]::new(
        'CN=SignPath Foundation, O=SignPath Foundation, C=US'
    )
    $validRequest = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new(
        $validSubject,
        $rsa,
        [System.Security.Cryptography.HashAlgorithmName]::SHA256,
        [System.Security.Cryptography.RSASignaturePadding]::Pkcs1
    )
    $validCertificate = $validRequest.CreateSelfSigned(
        [DateTimeOffset]::UtcNow.AddDays(-1),
        [DateTimeOffset]::UtcNow.AddDays(1)
    )
    $certificates.Add($validCertificate)
    Assert-NaygoSignPathPublisher -Certificate $validCertificate -ArtifactPath 'synthetic-valid.exe'

    foreach ($wrongPublisher in @('Microsoft Windows', 'signpath foundation')) {
        $subject = [System.Security.Cryptography.X509Certificates.X500DistinguishedName]::new(
            "CN=$wrongPublisher, O=$wrongPublisher, C=US"
        )
        $request = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new(
            $subject,
            $rsa,
            [System.Security.Cryptography.HashAlgorithmName]::SHA256,
            [System.Security.Cryptography.RSASignaturePadding]::Pkcs1
        )
        $certificate = $request.CreateSelfSigned(
            [DateTimeOffset]::UtcNow.AddDays(-1),
            [DateTimeOffset]::UtcNow.AddDays(1)
        )
        $certificates.Add($certificate)
        $rejected = $false
        try {
            Assert-NaygoSignPathPublisher -Certificate $certificate -ArtifactPath 'synthetic-invalid.exe'
        } catch {
            $rejected = $_.Exception.Message -match 'no es SignPath Foundation'
        }
        if (-not $rejected) {
            throw "El helper no rechazó el publicador '$wrongPublisher'."
        }
    }

    $temporaryFile = [System.IO.Path]::GetTempFileName()
    Set-Content -LiteralPath $temporaryFile -Value 'unsigned test artifact' -Encoding ascii
    $unsignedRejected = $false
    try {
        Assert-NaygoSignPathSignature -Path $temporaryFile
    } catch {
        $unsignedRejected = $_.Exception.Message -match 'Firma Authenticode inválida'
    }
    if (-not $unsignedRejected) {
        throw 'El helper no rechazó un artefacto sin firma.'
    }

    Write-Output 'Pruebas de firma pasaron: metadatos PE con padding Inno, rechazo de nombre/versión inválidos, identidad SignPath y rechazo de artefacto sin firma.'
} finally {
    if ($null -ne $temporaryFile -and (Test-Path -LiteralPath $temporaryFile -PathType Leaf)) {
        Remove-Item -LiteralPath $temporaryFile -Force
    }
    foreach ($certificate in $certificates) { $certificate.Dispose() }
    $rsa.Dispose()
}
