/**
 * AUTO_ALLOW_TOOLS - 不需要用戶確認的工具列表
 *
 * 這是 Tsunu Alive 權限系統的單一真相來源 (Single Source of Truth)
 *
 * 這些工具會自動允許執行，不會彈出權限確認對話框：
 * - 用戶互動工具：本身就是詢問用戶的工具
 * - 任務管理工具：內部追蹤用途
 * - 只讀工具：不修改檔案系統
 * - 子代理任務管理：Task 工具的內部運作
 *
 * 重要：如果修改此列表，也需要同步更新：
 * - src-tauri/src/permission_server.rs (AUTO_ALLOW_TOOLS 常數)
 *
 * 參考官方文件：
 * - https://code.claude.com/docs/en/settings
 *
 * @see permission_server.rs - Rust 後端的對應定義（需手動同步）
 */

// 用戶互動工具（本身就是詢問用戶）
const USER_INTERACTION_TOOLS = [
  'AskUserQuestion',
] as const;

// 任務管理工具（內部追蹤用途）
const TASK_MANAGEMENT_TOOLS = [
  'TodoRead',
  'TodoWrite',
] as const;

// 只讀工具（不修改檔案系統）
const READ_ONLY_TOOLS = [
  'Read',
  'Glob',
  'Grep',
] as const;

// 子代理任務管理
const SUBAGENT_TOOLS = [
  'Task',
  'TaskOutput',
] as const;

// 網路只讀工具
const WEB_READ_ONLY_TOOLS = [
  'WebSearch',
  'WebFetch',
] as const;

// Plan 模式相關（進入 Plan 模式不需要確認）
const PLAN_MODE_TOOLS = [
  'EnterPlanMode',
] as const;

/**
 * 完整的自動允許工具列表
 */
export const AUTO_ALLOW_TOOLS = [
  ...USER_INTERACTION_TOOLS,
  ...TASK_MANAGEMENT_TOOLS,
  ...READ_ONLY_TOOLS,
  ...SUBAGENT_TOOLS,
  ...WEB_READ_ONLY_TOOLS,
  ...PLAN_MODE_TOOLS,
] as const;

/**
 * 類型：自動允許的工具名稱
 */
export type AutoAllowToolName = typeof AUTO_ALLOW_TOOLS[number];

/**
 * 檢查工具是否在自動允許列表中
 */
export function isAutoAllowTool(toolName: string): boolean {
  return AUTO_ALLOW_TOOLS.includes(toolName as AutoAllowToolName);
}

// 導出分類列表（供除錯或顯示用）
export const AUTO_ALLOW_CATEGORIES = {
  userInteraction: USER_INTERACTION_TOOLS,
  taskManagement: TASK_MANAGEMENT_TOOLS,
  readOnly: READ_ONLY_TOOLS,
  subagent: SUBAGENT_TOOLS,
  webReadOnly: WEB_READ_ONLY_TOOLS,
  planMode: PLAN_MODE_TOOLS,
} as const;
