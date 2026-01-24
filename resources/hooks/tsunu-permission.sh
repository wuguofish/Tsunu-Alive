#!/bin/bash
# Tsunu Alive Permission Request Hook
# 這個腳本處理 Claude CLI 的 PermissionRequest Hook
# 將權限請求轉發到 Tsunu Alive 的 Permission HTTP Server

PERMISSION_SERVER_URL="http://localhost:19751/permission/request"

# 從 stdin 讀取輸入
INPUT=$(cat)

# 如果沒有輸入，退出讓 Claude CLI 顯示原生對話框
if [ -z "$INPUT" ]; then
    exit 1
fi

# 發送 HTTP 請求到 Tsunu Alive
# 設定較長的 timeout（55 秒），因為需要等待用戶決策
RESPONSE=$(echo "$INPUT" | curl -s -X POST "$PERMISSION_SERVER_URL" \
    -H "Content-Type: application/json; charset=utf-8" \
    --max-time 55 \
    -d @-)

# 檢查 curl 是否成功
if [ $? -ne 0 ]; then
    # 連線失敗，退出讓 Claude CLI 顯示原生對話框
    exit 1
fi

# 檢查回應是否為空
if [ -z "$RESPONSE" ]; then
    exit 1
fi

# 解析回應並建立 Hook 輸出格式
BEHAVIOR=$(echo "$RESPONSE" | jq -r '.behavior // empty')
MESSAGE=$(echo "$RESPONSE" | jq -r '.message // empty')

if [ -z "$BEHAVIOR" ]; then
    exit 1
fi

# 建立 Hook 回應 JSON
if [ -n "$MESSAGE" ]; then
    jq -n --arg behavior "$BEHAVIOR" --arg message "$MESSAGE" '{
        hookSpecificOutput: {
            hookEventName: "PermissionRequest",
            decision: {
                behavior: $behavior,
                message: $message
            }
        }
    }'
else
    jq -n --arg behavior "$BEHAVIOR" '{
        hookSpecificOutput: {
            hookEventName: "PermissionRequest",
            decision: {
                behavior: $behavior
            }
        }
    }'
fi
