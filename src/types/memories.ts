/**
 * 阿宇記憶系統型別定義
 *
 * 讓阿宇能夠跨 Claude Session 記住與使用者的「重要記憶」，
 * 實現真正的長期陪伴感。
 *
 * 儲存位置：~/.tsunu-alive/memories.json（全域）
 */

/** 記憶類型 */
export type MemoryType =
  | 'milestone'   // 里程碑：第一次使用某技術/框架、開始新專案
  | 'experience'  // 共同經歷：一起解決的困難問題、印象深刻的過程
  | 'growth'      // 成長軌跡：學會新技能、克服障礙
  | 'emotional';  // 情感連結：對話中分享的心情、重要的人生事件

/** 單筆記憶 */
export interface UniMemory {
  id: string;           // 唯一識別碼（nanoid）
  content: string;      // 記憶內容
  type: MemoryType;     // 記憶類型
  createdAt: string;    // ISO 8601 時間
  source: 'manual' | 'auto';  // 手動記錄 or Compact 自動提取
}

/** 記憶儲存格式 */
export interface UniMemoryStore {
  version: 1;
  memories: UniMemory[];
  lastUpdated: string;  // ISO 8601 時間
}

/** 記憶上限（建議 15-20 筆） */
export const MAX_MEMORIES = 20;

/** 建立空的記憶儲存 */
export function createEmptyMemoryStore(): UniMemoryStore {
  return {
    version: 1,
    memories: [],
    lastUpdated: new Date().toISOString(),
  };
}

/** 格式化記憶為可讀字串（用於注入 System Prompt） */
export function formatMemoriesForPrompt(memories: UniMemory[]): string {
  if (memories.length === 0) {
    return '';
  }

  const lines = memories.map(m => {
    const date = new Date(m.createdAt).toLocaleDateString('zh-TW');
    return `- ${date}：${m.content}`;
  });

  return `## 我們的共同記憶

以下是我們一起經歷過的重要時刻，請在適當的時機自然地提起：

${lines.join('\n')}
`;
}
