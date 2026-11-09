# verify_all.ps1 — MT4 32位DLL全链路9层自动验证
# ⚠️ 通达信是 64 位 (x86_64) / MT4 是 32 位 (i686) — 永不混淆
# =============================================
# PowerShell 7.6.3 (pwsh) or 5.1 (powershell) compatible
# UTF-8 BOM encoding required for 5.1 Chinese path support
#
# Layers:
#   L0: git diff audit
#   L1: cargo check --workspace
#   L2: cargo test --lib (判生死)
#   L3: cargo test (集成测试, 判生死)
#   L4: cargo test cross_dll_compare (跨DLL一致性)
#   L5: cargo build --release (生成 DLL)
#   L6: MQL4 compile (metaeditor.exe + ex4验证 + 退出码)
#   L7: Deployment sync (MT4目录文件时间戳 ≥ 构建文件)
#   L8: DLL exports (12个导出符号验证)
#   L9: MT4 manual test (人工)
#
# 用法: powershell -File verify_all.ps1
# 也可: powershell -File verify_all.ps1 -SkipBuild
#       powershell -File verify_all.ps1 -SkipMql4

param(
    [switch]$SkipBuild = $false,
    [switch]$SkipMql4 = $false
)

$ErrorActionPreference = "Continue"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Target = "i686-pc-windows-msvc"   # MT4 32-bit DLL

# ── 路径配置: 按你的 MT4 环境填写; 留空则自动探测 ──
$MetaEditor = ""    # metaeditor.exe 完整路径, 例: D:\MT4\metaeditor.exe (留空自动探测)
$Mt4Base = ""       # MT4 数据目录 MQL4 根, 例: C:\Users\<你>\AppData\Roaming\MetaQuotes\Terminal\<ID>\MQL4 (留空自动探测)

# 自动探测 MT4 数据目录 (AppData\Roaming\MetaQuotes\Terminal\<ID>\MQL4, 取最近使用)
if (-not $Mt4Base -or -not (Test-Path $Mt4Base)) {
    $mql4Base = Get-ChildItem "$env:APPDATA\MetaQuotes\Terminal" -Directory -ErrorAction SilentlyContinue |
        ForEach-Object { Join-Path $_.FullName "MQL4" } |
        Where-Object { Test-Path $_ } |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($mql4Base) { $Mt4Base = $mql4Base }
}
if (-not (Test-Path $Mt4Base)) {
    Write-Host "ERROR: MT4 数据目录未找到. 请配置 `$Mt4Base." -ForegroundColor Red
    exit 1
}

# 自动探测 metaeditor.exe (WebInstall 目录)
if (-not $MetaEditor -or -not (Test-Path $MetaEditor)) {
    $me = Get-ChildItem "$env:APPDATA\MetaQuotes\WebInstall" -Recurse -Filter "metaeditor.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($me) { $MetaEditor = $me.FullName }
}
if (-not (Test-Path $MetaEditor)) {
    Write-Host "ERROR: metaeditor.exe 未找到. 请配置 `$MetaEditor." -ForegroundColor Red
    exit 1
}
$Mql4Source  = Join-Path $Mt4Base "Indicators\MT4指标测试\chanlun.mq4"
$Mql4Log     = Join-Path $ScriptDir "mql4\compile.log"
$Mt4LibDll   = Join-Path $Mt4Base "Libraries\slzs_chanlun_mt4.dll"
$Mt4IndMq4   = Join-Path $Mt4Base "Indicators\MT4指标测试\chanlun.mq4"
$Mt4IndEx4   = Join-Path $Mt4Base "Indicators\MT4指标测试\chanlun.ex4"
$BuildDll    = Join-Path $ScriptDir "target\$Target\release\slzs_chanlun_mt4.dll"
$BuildEx4    = [System.IO.Path]::ChangeExtension($Mql4Source, ".ex4")

# ── 输出辅助 ──
function Write-Step($num, $desc) {
    Write-Host ""
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host "  Layer $num : $desc" -ForegroundColor Cyan
    Write-Host "============================================================" -ForegroundColor Cyan
}
function Write-Pass($msg) { Write-Host "  [PASS] $msg" -ForegroundColor Green }
function Write-Fail($msg) { Write-Host "  [FAIL] $msg" -ForegroundColor Red }
function Write-Info($msg) { Write-Host "  [INFO] $msg" -ForegroundColor Yellow }

$TotalPass = 0
$TotalFail = 0
$TotalSkip = 0
$StartTime = Get-Date

Write-Host ""
Write-Host "============================================================" -ForegroundColor Magenta
Write-Host "  ChanLun MT4 DLL - Full-Chain Verification v1.0" -ForegroundColor Magenta
Write-Host "  Principle: Machine-verifiable before human review" -ForegroundColor Magenta
Write-Host "============================================================" -ForegroundColor Magenta

# ════════════════════════════════════════════════════
# Layer 0: git diff audit
# ════════════════════════════════════════════════════
Write-Step 0 "git diff audit"
Push-Location $ScriptDir
$diffOutput = git diff --stat 2>&1
if ($LASTEXITCODE -eq 0) {
    if ([string]::IsNullOrWhiteSpace($diffOutput)) {
        Write-Info "No uncommitted changes"
    } else {
        Write-Info "Changes detected:"
        Write-Host $diffOutput
    }
    Write-Pass "git diff audit complete"
    $TotalPass += 1
} else {
    Write-Fail "git diff failed"
    $TotalFail += 1
}
Pop-Location

# ════════════════════════════════════════════════════
# Layer 1: cargo check
# ════════════════════════════════════════════════════
Write-Step 1 "cargo check - compile check"
Push-Location $ScriptDir
Write-Info "Running cargo check..."
$checkResult = cargo check 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Pass "cargo check passed"
    $TotalPass += 1
} else {
    Write-Fail "cargo check failed"
    Write-Host $checkResult
    $TotalFail += 1
    Pop-Location
    Write-Host ""
    Write-Host "!!! Stopping - fix compile errors first !!!" -ForegroundColor Red
    Pop-Location
    exit 1
}
Pop-Location

# ════════════════════════════════════════════════════
# Layer 2: cargo test --lib (unit tests)
# ════════════════════════════════════════════════════
Write-Step 2 "cargo test --lib - unit tests (life-or-death)"
Push-Location $ScriptDir
Write-Info "Running unit tests..."
$libTestResult = cargo test --lib 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Pass "All unit tests passed"
    $TotalPass += 1
} else {
    Write-Fail "Unit test(s) failed"
    $failLines = $libTestResult | Select-String "FAILED|failures:"
    Write-Host $failLines
    $TotalFail += 1
}
Pop-Location

# ════════════════════════════════════════════════════
# Layer 3: cargo test (integration tests)
# ════════════════════════════════════════════════════
Write-Step 3 "cargo test - integration tests (life-or-death)"
Push-Location $ScriptDir
Write-Info "Running all tests (including integration)..."
$fullTestResult = cargo test 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Pass "All tests passed (including integration)"
    $TotalPass += 1
} else {
    Write-Fail "Integration test(s) failed"
    $failLines = $fullTestResult | Select-String "FAILED|failures:"
    Write-Host $failLines
    $TotalFail += 1
}
Pop-Location

# ════════════════════════════════════════════════════
# Layer 4: Cross-DLL consistency
# ════════════════════════════════════════════════════
Write-Step 4 "cross-dll-compare - Cross-DLL consistency"
Push-Location $ScriptDir
Write-Info "Running cross-dll comparison tests..."
$crossResult = cargo test --test cross_dll_compare 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Pass "Cross-DLL consistency verified"
    $TotalPass += 1
} else {
    Write-Fail "Cross-DLL test(s) failed"
    $failLines = $crossResult | Select-String "FAILED|failures:"
    Write-Host $failLines
    $TotalFail += 1
}
Pop-Location

# ════════════════════════════════════════════════════
# Layer 5: cargo build --release
# ════════════════════════════════════════════════════
if ($SkipBuild) {
    Write-Step 5 "cargo build --release - SKIPPED (-SkipBuild flag)"
    Write-Info "Skipping release build"
    $TotalSkip += 1
} else {
    Write-Step 5 "cargo build --release --target $Target - Generate DLL"
    Push-Location $ScriptDir
    Write-Info "Building release DLL (may take ~60s)..."
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $buildResult = cargo build --release --target $Target 2>&1
    $sw.Stop()
    if ($LASTEXITCODE -eq 0) {
        $secs = [math]::Round($sw.Elapsed.TotalSeconds, 1)
        $msg = "Release build succeeded (" + $secs + "s)"
        Write-Pass $msg
        $TotalPass += 1
    } else {
        Write-Fail "Release build failed"
        Write-Host $buildResult
        $TotalFail += 1
        Pop-Location
        exit 1
    }
    Pop-Location
}

# ════════════════════════════════════════════════════
# Layer 6: MQL4 compile check
# ════════════════════════════════════════════════════
if ($SkipMql4) {
    Write-Step 6 "MQL4 compile - SKIPPED (-SkipMql4 flag)"
    Write-Info "Skipping MQL4 compilation"
    $TotalSkip += 1
} elseif (-not (Test-Path $MetaEditor)) {
    Write-Step 6 "MQL4 compile - SKIPPED (metaeditor.exe not found)"
    Write-Info ("Compiler not found: " + $MetaEditor)
    $TotalSkip += 1
} elseif (-not (Test-Path $Mql4Source)) {
    Write-Step 6 "MQL4 compile - SKIPPED (source not found)"
    Write-Info ("Source not found: " + $Mql4Source)
    $TotalSkip += 1
} else {
    Write-Step 6 "MQL4 compile - metaeditor.exe syntax check"
    Write-Info ("Source: " + $Mql4Source)
    Write-Info ("Compiler: " + $MetaEditor)

    # 强制重新编译: 删旧 ex4
    $ex4Path = [System.IO.Path]::ChangeExtension($Mql4Source, ".ex4")
    if (Test-Path $ex4Path) { Remove-Item $ex4Path -Force }
    if (Test-Path $Mql4Log)  { Remove-Item $Mql4Log -Force }

    # Compile (捕获退出码)
    Write-Info "Compiling..."
    $compileArgs = '/compile:"' + $Mql4Source + '" /log:"' + $Mql4Log + '"'
    $compileStdout = & $MetaEditor $compileArgs 2>&1
    $compileExit = $LASTEXITCODE
    Start-Sleep -Seconds 3

    # 检查编译器退出码 (非0=编译失败)
    if ($compileExit -ne 0) {
        Write-Fail ("metaeditor.exe exit code: " + $compileExit)
        # 仍然解析日志/输出以获取详细信息
    }

    # 解析编译结果 (优先读log, 失败则解析stdout, 清洗null字节)
    $hasError = $false
    $hasWarning = $false
    $resultLine = ""

    if (Test-Path $Mql4Log) {
        $logContent = Get-Content $Mql4Log -Raw
        if ($logContent -match "Result:\s*(\d+)\s*errors?,\s*(\d+)\s*warnings?") {
            $errorCount = [int]$Matches[1]
            $warnCount  = [int]$Matches[2]
            $resultLine = "Result: " + $errorCount + " errors, " + $warnCount + " warnings"
            if ($errorCount -gt 0) {
                Write-Fail ("Compile errors: " + $errorCount)
                $errorLines = Get-Content $Mql4Log | Select-String "error"
                Write-Host $errorLines
                $hasError = $true
            }
            if ($warnCount -gt 0) {
                Write-Info ("Compile warnings: " + $warnCount + " (non-fatal)")
                $warnLines = Get-Content $Mql4Log | Select-String "warning"
                foreach ($w in $warnLines) { Write-Info ("  " + $w.Line.Trim()) }
                $hasWarning = $true
            }
        } else {
            Write-Info "Compile log (no Result line):"
            Write-Host (Get-Content $Mql4Log -Raw)
        }
    } else {
        # Log未生成: 从stdout解析
        Write-Info "Compile log not generated, parsing stdout..."
        $stdoutStr = [string]($compileStdout -join "`n")
        if ($stdoutStr -match "Result:\s*(\d+)\s*errors?,\s*(\d+)\s*warnings?") {
            $errorCount = [int]$Matches[1]
            $warnCount  = [int]$Matches[2]
            $resultLine = "Result: " + $errorCount + " errors, " + $warnCount + " warnings (from stdout)"
            if ($errorCount -gt 0) {
                Write-Fail ("Compile errors: " + $errorCount)
                $hasError = $true
            }
            if ($warnCount -gt 0) {
                Write-Info ("Compile warnings: " + $warnCount + " (non-fatal)")
                $hasWarning = $true
            }
        } else {
            Write-Fail "Compile log not generated AND no Result line in stdout"
            Write-Host ($stdoutStr -replace "`0", "")
            $hasError = $true
        }
    }

    # 🔴 强制验证 .ex4 存在
    $ex4Ok = $false
    if (Test-Path $ex4Path) {
        $ex4Size = (Get-Item $ex4Path).Length
        if ($ex4Size -gt 100) {  # 至少100字节, 空ex4约7KB
            $ex4SizeKB = [math]::Round($ex4Size / 1024, 1)
            Write-Info ("Compiled: " + $ex4Path + " (" + $ex4SizeKB + " KB)")
            $ex4Ok = $true
        } else {
            Write-Fail (".ex4 too small: " + $ex4Size + " bytes")
        }
    } else {
        Write-Fail ".ex4 NOT GENERATED — MQL4 compile failed silently!"
        $hasError = $true
    }

    if ($hasError) {
        Write-Fail "MQL4 compilation FAILED"
        $TotalFail += 1
    } elseif (-not $ex4Ok) {
        Write-Fail "MQL4 .ex4 missing or invalid"
        $TotalFail += 1
    } else {
        if ($hasWarning) {
            Write-Pass ("MQL4 compiled with warnings (" + $resultLine + ")")
        } else {
            Write-Pass ("MQL4 compiled cleanly (" + $resultLine + ")")
        }
        $TotalPass += 1
    }
}

# ════════════════════════════════════════════════════
# Layer 7: Deployment sync check
# ════════════════════════════════════════════════════
Write-Step 7 "Deployment sync - MT4 files up-to-date"
Write-Info ("Build DLL:  " + $BuildDll)
Write-Info ("MT4  DLL:   " + $Mt4LibDll)
Write-Info ("Build ex4:  " + $BuildEx4)
Write-Info ("MT4  ex4:   " + $Mt4IndEx4)

$deployOk = $true

# 检查 DLL 部署
if (-not (Test-Path $Mt4LibDll)) {
    Write-Fail ("DLL not deployed: " + $Mt4LibDll)
    Write-Info "Run build_deploy.ps1 (close MT4 first)"
    $deployOk = $false
} elseif (-not (Test-Path $BuildDll)) {
    Write-Fail ("Build DLL missing: " + $BuildDll)
    $deployOk = $false
} else {
    $buildDllTime = (Get-Item $BuildDll).LastWriteTime
    $mt4DllTime   = (Get-Item $Mt4LibDll).LastWriteTime
    if ($mt4DllTime -ge $buildDllTime) {
        Write-Pass ("DLL synced (build " + $buildDllTime.ToString("HH:mm:ss") + " ≤ deploy " + $mt4DllTime.ToString("HH:mm:ss") + ")")
    } else {
        Write-Fail ("DLL STALE! build=" + $buildDllTime.ToString("HH:mm:ss") + " deploy=" + $mt4DllTime.ToString("HH:mm:ss"))
        $deployOk = $false
    }
}

# 检查 ex4 部署
if (-not (Test-Path $Mt4IndEx4)) {
    Write-Fail ("ex4 not deployed: " + $Mt4IndEx4)
    Write-Info "Run build_deploy.ps1 or copy manually"
    $deployOk = $false
} elseif (-not (Test-Path $BuildEx4)) {
    Write-Fail ("Build ex4 missing: " + $BuildEx4)
    $deployOk = $false
} else {
    $buildEx4Time = (Get-Item $BuildEx4).LastWriteTime
    $mt4Ex4Time   = (Get-Item $Mt4IndEx4).LastWriteTime
    if ($mt4Ex4Time -ge $buildEx4Time) {
        Write-Pass ("ex4 synced (build " + $buildEx4Time.ToString("HH:mm:ss") + " ≤ deploy " + $mt4Ex4Time.ToString("HH:mm:ss") + ")")
    } else {
        Write-Fail ("ex4 STALE! build=" + $buildEx4Time.ToString("HH:mm:ss") + " deploy=" + $mt4Ex4Time.ToString("HH:mm:ss"))
        $deployOk = $false
    }
}

# 检查 mq4 部署
if (-not (Test-Path $Mt4IndMq4)) {
    Write-Info ("mq4 not deployed: " + $Mt4IndMq4 + " (non-critical)")
} else {
    $srcMq4Time = (Get-Item $Mql4Source).LastWriteTime
    $mt4Mq4Time = (Get-Item $Mt4IndMq4).LastWriteTime
    if ($mt4Mq4Time -ge $srcMq4Time) {
        Write-Pass ("mq4 synced")
    } else {
        Write-Info ("mq4 source newer than deployed (non-critical, ex4 is the compiled form)")
    }
}

if ($deployOk) {
    $TotalPass += 1
} else {
    Write-Fail "Deployment check FAILED — MT4 may be running stale code!"
    $TotalFail += 1
}

# ════════════════════════════════════════════════════
# Layer 8: DLL export verification
# ════════════════════════════════════════════════════
Write-Step 8 "DLL exports - Export symbol verification"
$DllPath = Join-Path $ScriptDir "target\$Target\release\slzs_chanlun_mt4.dll"

if (-not (Test-Path $DllPath)) {
    Write-Fail "DLL not found: $DllPath"
    Write-Info "Run without -SkipBuild or fix build errors first"
    $TotalFail += 1
} else {
    $dllInfo = Get-Item $DllPath
    $sizeKB = [math]::Round($dllInfo.Length / 1024, 1)
    Write-Info ("DLL: " + $DllPath + " (" + $sizeKB + " KB)")

    # 用字符串搜索验证导出符号 (dumpbin 不在 PATH)
    $bytes = [System.IO.File]::ReadAllBytes($DllPath)
    $text = [System.Text.Encoding]::ASCII.GetString($bytes)
    $expected = @(
        "chanlun_init",
        "chanlun_get_strokes",
        "chanlun_get_segments",
        "chanlun_get_bigsegments",
        "chanlun_get_stroke_bands",
        "chanlun_get_segment_bands",
        "chanlun_get_bigseg_bands",
        "chanlun_get_superior_segments",
        "chanlun_markers_compute",
        "chanlun_markers_get",
        "chanlun_zhongshus_compute",
        "chanlun_zhongshus_get"
    )
    $missing = @()
    foreach ($f in $expected) {
        if ($text.Contains($f)) {
            Write-Pass ("  Export found: " + $f)
        } else {
            Write-Fail ("  Export MISSING: " + $f)
            $missing += $f
        }
    }
    if ($missing.Count -eq 0) {
        Write-Pass "All 12 exported functions verified"
        $TotalPass += 1
    } else {
        Write-Fail ("Missing exports: " + ($missing -join ", "))
        $TotalFail += 1
    }
}

# ════════════════════════════════════════════════════
# Layer 9: MT4 manual test
# ════════════════════════════════════════════════════
Write-Step 9 "MT4 terminal manual test - HUMAN REQUIRED"
Write-Info "Steps:"
Write-Info "  1. Close MT4 terminal"
Write-Info "  2. Run build_deploy.ps1 (DLL + MQL4 auto-deployed)"
Write-Info "  3. Start MT4 → Indicators/MT4指标测试/ → chanlun"
Write-Info "  4. Visually compare DLL vs reference output"
$TotalSkip += 1

# ════════════════════════════════════════════════════
# Summary
# ════════════════════════════════════════════════════
$EndTime = Get-Date
$elapsedSecs = [math]::Round(($EndTime - $StartTime).TotalSeconds, 1)

Write-Host ""
Write-Host "============================================================" -ForegroundColor Magenta
Write-Host "  VERIFICATION RESULTS" -ForegroundColor Magenta
Write-Host "============================================================" -ForegroundColor Magenta
$summaryLine = "  PASS: " + $TotalPass + "  |  FAIL: " + $TotalFail + "  |  SKIP: " + $TotalSkip + "  |  TIME: " + $elapsedSecs + "s"
Write-Host $summaryLine -ForegroundColor White
Write-Host "============================================================" -ForegroundColor Magenta

if ($TotalFail -gt 0) {
    Write-Host ""
    Write-Host ("!!! VERIFICATION FAILED - " + $TotalFail + " layer(s) have errors !!!") -ForegroundColor Red
    Write-Host "Fix failures before handing off to user." -ForegroundColor Red
    exit 1
} else {
    Write-Host ""
    Write-Host "ALL CHECKS PASSED - Ready for manual Layer 9 (MT4 test)" -ForegroundColor Green
    exit 0
}
