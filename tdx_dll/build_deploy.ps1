# Build + Deploy slzs_chanlun.dll to TDX (64-bit DLL)
# 构建脚本: 在本仓库 (chanlun_kaiyuan) 上构建并部署到本机通达信
# TDX 仅支持 64 位 DLL → 使用默认 x86_64-pc-windows-mscv
# ⚠️ 通达信是 64 位 (x86_64) / MT4 是 32 位 (i686) — 永不混淆
# Usage: powershell -File build_deploy.ps1

$ErrorActionPreference = "Stop"

$ProjectDir = Split-Path -Parent $MyInvocation.MyCommand.Path   # 开源副本 (chanlun_kaiyuan/tdx_dll)
$DllName = "slzs_chanlun.dll"

# ── 路径配置: 按你的通达信安装环境修改 ──
$TdxDllDir = ""    # 你的通达信 T0002\dlls 目录, 例: D:\通达信\T0002\dlls
if (-not $TdxDllDir -or -not (Test-Path $TdxDllDir)) {
    Write-Host "ERROR: 请先在脚本开头配置 `$TdxDllDir 为你的通达信 T0002\dlls 目录." -ForegroundColor Red
    exit 1
}

Write-Host "=== Chanlun DLL: Build + Deploy ===" -ForegroundColor Cyan

# Step 1: Build (64-bit, default x86_64-pc-windows-mscv)
Write-Host "[1/4] cargo build --release (64-bit) ..." -ForegroundColor Yellow
Push-Location $ProjectDir
$buildResult = cargo build --release 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "BUILD FAILED!" -ForegroundColor Red
    Write-Host $buildResult
    Pop-Location
    exit 1
}
Write-Host "[OK] Build succeeded" -ForegroundColor Green

# Step 2: Test
Write-Host "[2/4] cargo test ..." -ForegroundColor Yellow
$testResult = cargo test 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "TEST FAILED!" -ForegroundColor Red
    Write-Host $testResult
    Pop-Location
    exit 1
}
Write-Host "[OK] Tests passed" -ForegroundColor Green
Pop-Location

# Step 3: Copy to TDX
$src = Join-Path $ProjectDir "target\release\$DllName"
$dst = Join-Path $TdxDllDir $DllName

if (-not (Test-Path $src)) {
    Write-Host "DLL NOT FOUND: $src" -ForegroundColor Red
    exit 1
}

Write-Host "[3/4] Deploy to: $dst" -ForegroundColor Yellow
Copy-Item -Path $src -Destination $dst -Force
Write-Host "[OK] Deployed" -ForegroundColor Green

# Step 4: Verify consistency
Write-Host "[4/4] Verify version consistency..." -ForegroundColor Yellow
$srcTime = (Get-Item $src).LastWriteTime
$dstTime = (Get-Item $dst).LastWriteTime

if ($srcTime -eq $dstTime) {
    Write-Host "[OK] Version consistent ($($srcTime.ToString('HH:mm:ss')))" -ForegroundColor Green
} else {
    Write-Host "TIMESTAMP MISMATCH! src=$srcTime dst=$dstTime" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "=== Deploy Complete ===" -ForegroundColor Cyan
Write-Host "DLL:  $dst" -ForegroundColor White
Write-Host "Time: $($srcTime.ToString('yyyy-MM-dd HH:mm:ss'))" -ForegroundColor White
Write-Host ""
Write-Host "WARNING: Close TDX before overwriting DLL!" -ForegroundColor Yellow
