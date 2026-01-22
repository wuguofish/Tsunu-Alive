// Vitest 全域設定
// 這個檔案會在所有測試前執行

// Mock Tauri API（測試時不會有 Tauri 環境）
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))
