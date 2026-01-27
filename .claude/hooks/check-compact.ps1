#!/usr/bin/env pwsh
# check-compact.ps1 - Compact 後自動觸發記憶提取
#
# UserPromptSubmit hook：檢測 Compact 摘要並注入記憶提取指令
# 當檢測到新的 Compact 時，輸出指令讓 Claude 提取值得記住的內容

# 從 stdin 讀取 hook 資料
$hookData = [Console]::In.ReadToEnd() | ConvertFrom-Json

$transcriptPath = $hookData.transcript_path
$sessionId = $hookData.session_id

# 確保 transcript 檔案存在
if (-not (Test-Path $transcriptPath)) {
    exit 0
}

# 讀取 transcript 內容
$transcriptContent = Get-Content $transcriptPath -Raw -ErrorAction SilentlyContinue
if (-not $transcriptContent) {
    exit 0
}

# 檢測 Compact 標記
$compactMarker = "This session is being continued from a previous conversation that ran out of context"
if ($transcriptContent -notmatch [regex]::Escape($compactMarker)) {
    exit 0
}

# 提取 Compact 摘要（從標記開始的前 2000 字元）
$markerIndex = $transcriptContent.IndexOf($compactMarker)
$summaryEnd = [Math]::Min($markerIndex + 2000, $transcriptContent.Length)
$compactSummary = $transcriptContent.Substring($markerIndex, $summaryEnd - $markerIndex)

# 計算 hash
$hashBytes = [System.Security.Cryptography.SHA256]::Create().ComputeHash(
    [System.Text.Encoding]::UTF8.GetBytes($compactSummary)
)
$currentHash = [BitConverter]::ToString($hashBytes) -replace '-', ''

# 檢查是否已處理過（避免重複提取）
$tsuNuAliveDir = Join-Path $env:USERPROFILE ".tsunu-alive"
$hashFile = Join-Path $tsuNuAliveDir "compact_hash_$sessionId.txt"

if (-not (Test-Path $tsuNuAliveDir)) {
    New-Item -ItemType Directory -Path $tsuNuAliveDir -Force | Out-Null
}

if (Test-Path $hashFile) {
    $storedHash = Get-Content $hashFile -Raw -ErrorAction SilentlyContinue
    if ($storedHash -and $storedHash.Trim() -eq $currentHash) {
        # 已處理過這個 Compact，不重複注入
        exit 0
    }
}

# 記錄 hash
Set-Content -Path $hashFile -Value $currentHash -NoNewline

# 輸出記憶提取指令到 stdout（會被注入到 Claude context）
@"

[System Reminder] 剛才發生了 Compact（對話摘要壓縮）。請用阿宇的視角檢查壓縮摘要中是否有值得長期記住的內容：

1. **里程碑**：第一次使用某技術/框架、開始新專案
2. **共同經歷**：一起解決的困難問題、印象深刻的 debug 過程
3. **成長軌跡**：學會新技能、克服障礙
4. **情感連結**：對話中分享的心情、重要的人生事件

如果有值得記住的內容，請在回應末尾用 <memory-update> 標籤輸出（用阿宇的視角，第三人稱描述使用者）：

<memory-update>
- [type:experience] 使用者今天終於把權限系統做完了，過程中遇到不少困難但堅持下來了
- [type:milestone] 這是使用者第一次用 Rust 寫後端
</memory-update>

type 可選值：milestone, experience, growth, emotional

不需要記錄的：純技術問答、一般性的程式碼修改、沒有特別意義的日常操作。
如果沒有值得記錄的內容，就不要輸出 <memory-update> 標籤。

"@
