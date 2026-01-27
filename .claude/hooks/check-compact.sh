#!/bin/bash
# check-compact.sh - Compact 後自動觸發記憶提取
#
# UserPromptSubmit hook：檢測 Compact 摘要並注入記憶提取指令
# 當檢測到新的 Compact 時，輸出指令讓 Claude 提取值得記住的內容

# 從 stdin 讀取 hook 資料
HOOK_DATA=$(cat)
TRANSCRIPT_PATH=$(echo "$HOOK_DATA" | jq -r '.transcript_path // empty')
SESSION_ID=$(echo "$HOOK_DATA" | jq -r '.session_id // empty')

# 確保 transcript 檔案存在
if [ -z "$TRANSCRIPT_PATH" ] || [ ! -f "$TRANSCRIPT_PATH" ]; then
    exit 0
fi

# 檢測 Compact 標記
COMPACT_MARKER="This session is being continued from a previous conversation that ran out of context"
if ! grep -q "$COMPACT_MARKER" "$TRANSCRIPT_PATH" 2>/dev/null; then
    exit 0
fi

# 提取 Compact 摘要（從標記開始的前 100 行）
COMPACT_SUMMARY=$(grep -A 100 "$COMPACT_MARKER" "$TRANSCRIPT_PATH" | head -n 100)

# 計算 hash
CURRENT_HASH=$(echo "$COMPACT_SUMMARY" | sha256sum | cut -d' ' -f1)

# 檢查是否已處理過（避免重複提取）
TSUNU_ALIVE_DIR="$HOME/.tsunu-alive"
HASH_FILE="$TSUNU_ALIVE_DIR/compact_hash_${SESSION_ID}.txt"

mkdir -p "$TSUNU_ALIVE_DIR"

if [ -f "$HASH_FILE" ]; then
    STORED_HASH=$(cat "$HASH_FILE")
    if [ "$STORED_HASH" = "$CURRENT_HASH" ]; then
        # 已處理過這個 Compact，不重複注入
        exit 0
    fi
fi

# 記錄 hash
echo -n "$CURRENT_HASH" > "$HASH_FILE"

# 輸出記憶提取指令到 stdout（會被注入到 Claude context）
cat << 'EOF'

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

EOF
