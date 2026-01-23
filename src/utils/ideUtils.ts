/**
 * IDE 整合相關的工具函數
 */

export interface IdeSelection {
  start_line: number;
  start_character: number;
  end_line: number;
  end_character: number;
}

export interface IdeContext {
  file_path: string | null;
  selected_text: string | null;
  selection: IdeSelection | null;
  language_id: string | null;
  last_updated: string | null;
}

/**
 * 從 IDE context 生成檔案參考字串
 * 格式: @filepath#L1-10 或 @filepath#L5 或 @filepath
 */
export function generateFileReference(ctx: IdeContext | null): string | null {
  if (!ctx?.file_path) return null;

  let reference = `@${ctx.file_path}`;

  if (ctx.selection) {
    const startLine = ctx.selection.start_line + 1; // 轉為 1-based
    const endLine = ctx.selection.end_line + 1;

    if (startLine === endLine) {
      reference += `#L${startLine}`;
    } else {
      reference += `#L${startLine}-${endLine}`;
    }
  }

  return reference;
}

/**
 * 從檔案路徑提取檔案名稱
 */
export function extractFileName(filePath: string | null): string {
  if (!filePath) return '';
  // 支援 Windows 和 Unix 路徑
  return filePath.split(/[\\/]/).pop() || '';
}

/**
 * 生成 IDE context 的顯示文字
 * 格式: filename.ts:123 或 filename.ts
 */
export function formatContextDisplay(ctx: IdeContext | null): string | null {
  if (!ctx?.file_path) return null;

  const fileName = extractFileName(ctx.file_path);

  if (ctx.selection) {
    return `${fileName}:${ctx.selection.start_line + 1}`;
  }

  return fileName;
}

/**
 * 計算 IDE 連接狀態文字
 */
export function getConnectionStatusText(
  running: boolean,
  clients: Array<{ name: string }>,
): string {
  if (!running) return 'IDE: Off';
  if (clients.length === 0) return 'IDE: Waiting';
  if (clients.length === 1) return `IDE: ${clients[0].name}`;
  return `IDE: ${clients.length} connected`;
}
