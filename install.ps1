$ErrorActionPreference = "Stop"

$repo = "pholgy/fiq"
$installDir = if ($env:FIQ_INSTALL_DIR) { $env:FIQ_INSTALL_DIR } else { "$env:USERPROFILE\.local\bin" }
$asset = "fiq-windows-x86_64.zip"

Write-Host "Fetching latest release..."
$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest"
$tag = $release.tag_name

if (-not $tag) {
    Write-Error "Could not determine latest release"
    exit 1
}

$url = "https://github.com/$repo/releases/download/$tag/$asset"

Write-Host "Downloading fiq $tag (windows/x86_64)..."
$tmpDir = Join-Path $env:TEMP "fiq-install-$(Get-Random)"
New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

try {
    $zipPath = Join-Path $tmpDir $asset
    Invoke-WebRequest -Uri $url -OutFile $zipPath

    Write-Host "Installing to $installDir..."
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Expand-Archive -Path $zipPath -DestinationPath $tmpDir -Force
    Move-Item -Path (Join-Path $tmpDir "fiq.exe") -Destination (Join-Path $installDir "fiq.exe") -Force

    # Check if install dir is in PATH
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ($userPath -notlike "*$installDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$installDir;$userPath", "User")
        Write-Host ""
        Write-Host "Added $installDir to your user PATH."
        Write-Host "Restart your terminal for the change to take effect."
    }

    Write-Host ""
    Write-Host "fiq $tag installed to $installDir\fiq.exe"
}
finally {
    Remove-Item -Path $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
