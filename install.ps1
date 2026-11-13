# Install BELL Suite on Windows (PowerShell native or WSL2).
# Usage: .\install.ps1 [-Version latest] [-Lang en|zh-TW] [-Wsl] [-Distribution Ubuntu]
[CmdletBinding()]
param(
    [string]$Version = 'latest',
    [ValidateSet('en', 'zh-TW')]
    [string]$Lang = 'en',
    [switch]$Wsl,
    [string]$Distribution = 'Ubuntu'
)

$ErrorActionPreference = 'Stop'
$PackageName = 'bell-suite'
$ProductName = 'BELL Suite'
$BinName = 'bell-suite'
$RepoUrl = 'https://github.com/Suzu1Dev/bell-suite.git'
$RawInstallSh = 'https://raw.githubusercontent.com/Suzu1Dev/bell-suite/main/install.sh'

$Messages = @{
    en = @{
        NeedNode = "Node.js 20+ and npm (or WSL2) are required. Install Node.js LTS from https://nodejs.org or pass -Wsl."
        InstallingNpm = "Installing $ProductName ($PackageName@$Version) via npm..."
        InstallingGit = "Package not yet published on npm; installing $ProductName from GitHub ($RepoUrl)..."
        NeedGit = "git is required for GitHub fallback installation. Install Git from https://git-scm.com."
        Done = "$ProductName is installed! Run it with:"
        Doctor = "Verify your setup with:"
    }
    'zh-TW' = @{
        NeedNode = "需要 Node.js 20+ 與 npm（或 WSL2）。請從 https://nodejs.org 安裝 Node.js LTS，或加上 -Wsl 參數。"
        InstallingNpm = "正在透過 npm 安裝 $ProductName ($PackageName@$Version)..."
        InstallingGit = "npm 尚未發布此套件，正在從 GitHub ($RepoUrl) 安裝 $ProductName..."
        NeedGit = "從 GitHub 安裝需要 git，請先至 https://git-scm.com 安裝。"
        Done = "$ProductName 安裝完成！請執行："
        Doctor = "檢查環境狀態："
    }
}
$Text = $Messages[$Lang]

if ($Wsl -or -not (Get-Command node -ErrorAction SilentlyContinue)) {
    if (Get-Command wsl.exe -ErrorAction SilentlyContinue) {
        $Tmp = [System.IO.Path]::GetTempFileName()
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $RawInstallSh -OutFile $Tmp
            $LinuxPath = (& wsl.exe -d $Distribution -- wslpath -a -u $Tmp | Out-String).Trim()
            & wsl.exe -d $Distribution -- sh $LinuxPath --version $Version --lang $Lang
            exit $LASTEXITCODE
        } finally {
            Remove-Item -LiteralPath $Tmp -Force -ErrorAction SilentlyContinue
        }
    }
    Write-Host $Text.NeedNode
    exit 1
}

$Spec = "$PackageName@$Version"
$Published = $false
try {
    $null = & npm view $Spec version 2>$null
    if ($LASTEXITCODE -eq 0) { $Published = $true }
} catch {}

if ($Published) {
    Write-Host $Text.InstallingNpm
    & npm install --global --no-audit --no-fund $Spec
    if ($LASTEXITCODE -eq 0) {
        Write-Host $Text.Done
        Write-Host "  $BinName"
        Write-Host $Text.Doctor
        Write-Host "  $BinName doctor"
        exit 0
    }
}

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Host $Text.NeedGit
    exit 1
}

Write-Host $Text.InstallingGit
$BaseDir = Join-Path $env:LOCALAPPDATA $BinName
$AppDir = Join-Path $BaseDir 'app'
$BinDir = Join-Path $env:USERPROFILE '.local\bin'
New-Item -ItemType Directory -Force -Path $BaseDir, $BinDir | Out-Null

if (Test-Path -LiteralPath (Join-Path $AppDir '.git')) {
    & git -C $AppDir fetch --depth 1 origin main
    & git -C $AppDir reset --hard FETCH_HEAD
} else {
    if (Test-Path -LiteralPath $AppDir) { Remove-Item -Recurse -Force $AppDir }
    & git clone --depth 1 $RepoUrl $AppDir
}
if ($LASTEXITCODE -ne 0) { throw "git clone/fetch failed" }

Push-Location $AppDir
try {
    & npm install --no-audit --no-fund
    if ($LASTEXITCODE -ne 0) { throw "npm install failed" }
    $Pkg = Get-Content -Raw -Path 'package.json' | ConvertFrom-Json
    if ($Pkg.scripts -and $Pkg.scripts.build) {
        & npm run build
        if ($LASTEXITCODE -ne 0) { throw "npm run build failed" }
    }
} finally {
    Pop-Location
}

$CmdShim = Join-Path $BinDir "$BinName.cmd"
"@echo off`r`nnode `"$AppDir\bin\$BinName.mjs`" %*`r`n" | Set-Content -Encoding ASCII -Path $CmdShim

Write-Host $Text.Done
Write-Host "  `"$CmdShim`""
Write-Host $Text.Doctor
Write-Host "  `"$CmdShim`" doctor"
