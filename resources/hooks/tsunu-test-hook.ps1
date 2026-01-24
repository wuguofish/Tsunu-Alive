# Tsunu Alive Test Hook - UserPromptSubmit
# 測試 Headless/JSON 模式下 Hook 是否能觸發

param()

$ErrorActionPreference = "Stop"

# 日誌檔案
$LOG_FILE = "$env:TEMP\tsunu-hook-test.log"

function Write-Log {
    param([string]$Message)
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss.fff"
    "$timestamp - $Message" | Out-File -FilePath $LOG_FILE -Append -Encoding UTF8
}

try {
    Write-Log "=========================================="
    Write-Log "UserPromptSubmit Hook TRIGGERED!"
    Write-Log "=========================================="

    # 從 stdin 讀取 JSON 輸入
    $inputJson = [Console]::In.ReadToEnd()

    if (-not [string]::IsNullOrWhiteSpace($inputJson)) {
        $hookInput = $inputJson | ConvertFrom-Json
        Write-Log "Session ID: $($hookInput.session_id)"
        Write-Log "Hook Event: $($hookInput.hook_event_name)"
        Write-Log "CWD: $($hookInput.cwd)"
        Write-Log "Raw Input Length: $($inputJson.Length) chars"
    } else {
        Write-Log "No input received (empty stdin)"
    }

    Write-Log "Hook completed successfully, passing through..."

    # Exit 0 = 成功，繼續執行
    exit 0

} catch {
    Write-Log "ERROR: $_"
    Write-Log "Exception: $($_.Exception.Message)"
    # 即使出錯也 exit 0，不阻擋執行
    exit 0
}
