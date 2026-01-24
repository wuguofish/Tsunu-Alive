# Tsunu Alive Permission Request Hook
# 這個腳本處理 Claude CLI 的 PermissionRequest Hook
# 將權限請求轉發到 Tsunu Alive 的 Permission HTTP Server

param()

$ErrorActionPreference = "Stop"

# Permission Server 的 URL
$PERMISSION_SERVER_URL = "http://localhost:19751/permission/request"

# 診斷日誌檔案（可選，用於調試）
$LOG_FILE = "$env:TEMP\tsunu-hook.log"

function Write-Log {
    param([string]$Message)
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    "$timestamp - $Message" | Out-File -FilePath $LOG_FILE -Append -Encoding UTF8
}

try {
    Write-Log "Hook script started"

    # 從 stdin 讀取 JSON 輸入
    $inputJson = [Console]::In.ReadToEnd()

    if ([string]::IsNullOrWhiteSpace($inputJson)) {
        # 如果沒有輸入，退出讓 Claude CLI 顯示原生對話框
        Write-Log "No input received, exiting with code 1"
        exit 1
    }

    $hookInput = $inputJson | ConvertFrom-Json
    Write-Log "Received request for tool: $($hookInput.tool_name), id: $($hookInput.tool_use_id)"

    # AskUserQuestion 需要特殊處理：
    # 這個工具需要用戶輸入答案，不只是「允許/拒絕」
    # Hook 機制無法處理用戶輸入，所以跳過讓 fallback 模式處理
    if ($hookInput.tool_name -eq "AskUserQuestion") {
        Write-Log "AskUserQuestion detected, skipping to fallback mode"
        exit 1
    }

    # 建立請求 body
    $body = @{
        tool_name = $hookInput.tool_name
        tool_input = $hookInput.tool_input
        tool_use_id = $hookInput.tool_use_id
        session_id = $hookInput.session_id
    } | ConvertTo-Json -Depth 10 -Compress

    Write-Log "Sending HTTP request to $PERMISSION_SERVER_URL"

    # 發送 HTTP 請求到 Tsunu Alive
    # 設定較長的 timeout（55 秒），因為需要等待用戶決策
    $response = Invoke-RestMethod -Uri $PERMISSION_SERVER_URL `
        -Method Post `
        -Body $body `
        -ContentType "application/json; charset=utf-8" `
        -TimeoutSec 55

    Write-Log "Received response: behavior=$($response.behavior)"

    # 建立 Hook 回應格式
    $hookOutput = @{
        hookSpecificOutput = @{
            hookEventName = "PermissionRequest"
            decision = @{
                behavior = $response.behavior
            }
        }
    }

    # 如果有 message，加入到 decision 中
    if ($response.message) {
        $hookOutput.hookSpecificOutput.decision.message = $response.message
    }

    # 如果有 updated_input，加入到 decision 中
    if ($response.updated_input) {
        $hookOutput.hookSpecificOutput.decision.updatedInput = $response.updated_input
    }

    # 輸出 JSON 給 Claude CLI
    $hookOutput | ConvertTo-Json -Depth 10 -Compress

} catch {
    # 連線失敗或其他錯誤，退出讓 Claude CLI 顯示原生對話框
    # 這樣可以確保即使 Tsunu Alive 沒有啟動，Claude CLI 仍然可以正常使用
    Write-Log "ERROR: $_"
    Write-Log "Exception details: $($_.Exception.Message)"
    Write-Error "Permission hook error: $_" 2>&1 | Out-Null
    exit 1
}
