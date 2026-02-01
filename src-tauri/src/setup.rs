// setup.rs - 首次啟動安裝精靈的後端指令
//
// 負責：
// - 檢測 VS Code Extension 和 Claude Code Skill 的安裝狀態
// - 從 bundled resources 安裝附加組件
// - 管理首次啟動標記

use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tauri::Manager;

/// 附加組件安裝狀態
#[derive(Debug, Clone, Serialize)]
pub struct AddonStatus {
    /// VS Code CLI 是否可用
    pub vscode_available: bool,
    /// VS Code Extension 是否已安裝
    pub vscode_installed: bool,
    /// Claude Code CLI 是否可用
    pub claude_available: bool,
    /// Claude Code Skill 是否已安裝
    pub skill_installed: bool,
}

/// 取得使用者 home 目錄
fn home_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

/// 尋找 VS Code CLI 路徑
fn find_vscode_cli() -> Option<String> {
    // Windows: use `where` to get the full path (handles .cmd extension)
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = Command::new("where").arg("code").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // `where` may return multiple lines; prefer .cmd
                for line in stdout.lines() {
                    let path = line.trim();
                    if path.ends_with(".cmd") {
                        return Some(path.to_string());
                    }
                }
                // Fallback to first result
                if let Some(first) = stdout.lines().next() {
                    let path = first.trim();
                    if !path.is_empty() {
                        return Some(path.to_string());
                    }
                }
            }
        }

        // Fallback: check common install locations
        let local_app_data = std::env::var("LOCALAPPDATA").ok()?;
        let paths = [
            format!("{}\\Programs\\Microsoft VS Code\\bin\\code.cmd", local_app_data),
            format!("{}\\Programs\\Microsoft VS Code\\Code.exe", local_app_data),
        ];
        for path in &paths {
            if std::path::Path::new(path).exists() {
                return Some(path.clone());
            }
        }
    }

    // Unix: use `which`
    #[cfg(not(target_os = "windows"))]
    {
        if which_exists("code") {
            return Some("code".to_string());
        }
    }

    // macOS fallback
    #[cfg(target_os = "macos")]
    {
        let path = "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code";
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    None
}

/// 檢查指令是否存在於 PATH 中
fn which_exists(cmd: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        Command::new("where")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// 檢查 VS Code Extension 是否已安裝
fn check_vscode_extension(code_cli: &str) -> bool {
    Command::new(code_cli)
        .args(["--list-extensions"])
        .output()
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines().any(|line| {
                line.trim().eq_ignore_ascii_case("tsunu-alive.tsunu-alive-connector")
            })
        })
        .unwrap_or(false)
}

/// 檢查 Claude Code Skill 是否已安裝
fn check_skill_installed() -> bool {
    if let Some(home) = home_dir() {
        home.join(".claude").join("skills").join("uni").join("SKILL.md").exists()
    } else {
        false
    }
}

/// 取得 bundled resources 目錄
fn get_bundled_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resource_dir()
        .map(|p: PathBuf| p.join("resources").join("bundled"))
        .map_err(|e| format!("Failed to get resource dir: {}", e))
}

// ========================
// Tauri Commands
// ========================

/// 檢查附加組件安裝狀態
#[tauri::command]
pub fn check_addon_status() -> AddonStatus {
    let vscode_cli = find_vscode_cli();
    let vscode_available = vscode_cli.is_some();
    let vscode_installed = vscode_cli
        .as_deref()
        .map(check_vscode_extension)
        .unwrap_or(false);

    let claude_available = which_exists("claude");
    let skill_installed = check_skill_installed();

    AddonStatus {
        vscode_available,
        vscode_installed,
        claude_available,
        skill_installed,
    }
}

/// 取得 VS Code 的 settings.json 路徑
fn get_vscode_settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA").ok()
            .map(|appdata| PathBuf::from(appdata).join("Code").join("User").join("settings.json"))
    }
    #[cfg(target_os = "macos")]
    {
        home_dir().map(|home| {
            home.join("Library")
                .join("Application Support")
                .join("Code")
                .join("User")
                .join("settings.json")
        })
    }
    #[cfg(target_os = "linux")]
    {
        home_dir().map(|home| {
            home.join(".config")
                .join("Code")
                .join("User")
                .join("settings.json")
        })
    }
}

/// 在 VS Code settings.json 中設定 tsunuAlive.executablePath
fn configure_vscode_executable_path() -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get current exe path: {}", e))?;
    let exe_str = exe_path.to_string_lossy().to_string();

    let settings_path = get_vscode_settings_path()
        .ok_or_else(|| "Cannot determine VS Code settings.json path".to_string())?;

    // 讀取現有設定（若不存在則用空物件）
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings.json: {}", e))?;
        // 嘗試解析 JSON（VS Code settings 可能有 trailing comma 等，先試 serde）
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        // 確保目錄存在
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create settings dir: {}", e))?;
        }
        serde_json::json!({})
    };

    // 設定 executablePath
    if let Some(obj) = settings.as_object_mut() {
        obj.insert(
            "tsunuAlive.executablePath".to_string(),
            serde_json::Value::String(exe_str),
        );
    }

    // 寫回（pretty print）
    let output = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&settings_path, output)
        .map_err(|e| format!("Failed to write settings.json: {}", e))?;

    Ok(())
}

/// 安裝 VS Code Extension（從 bundled resource）
#[tauri::command]
pub fn install_vscode_extension(app: tauri::AppHandle) -> Result<String, String> {
    let code_cli = find_vscode_cli()
        .ok_or_else(|| "VS Code CLI not found".to_string())?;

    let bundled_dir = get_bundled_dir(&app)?;
    let vsix_path = bundled_dir.join("tsunu-alive-connector.vsix");

    if !vsix_path.exists() {
        return Err(format!("VSIX file not found: {:?}", vsix_path));
    }

    let output = Command::new(&code_cli)
        .args(["--install-extension", &vsix_path.to_string_lossy(), "--force"])
        .output()
        .map_err(|e| format!("Failed to run VS Code CLI: {}", e))?;

    if output.status.success() {
        // Extension 安裝成功後，自動設定執行檔路徑
        if let Err(e) = configure_vscode_executable_path() {
            eprintln!("Warning: extension installed but failed to configure path: {}", e);
            // 不影響整體安裝結果，只是路徑需要手動設定
        }
        Ok("VS Code Extension installed successfully".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Installation failed: {}", stderr))
    }
}

/// 設定全域 ~/.claude/settings.json（讓 hooks 在所有專案都生效）
fn configure_global_claude_settings(home: &PathBuf, hooks_dir: &PathBuf) -> Result<(), String> {
    let settings_path = home.join(".claude").join("settings.json");

    // 讀取現有設定
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read global settings.json: {}", e))?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // 建構 hook command（使用絕對路徑）
    let hook_script = hooks_dir.join("check-compact.ps1");
    let hook_command = if cfg!(target_os = "windows") {
        format!(
            "powershell.exe -ExecutionPolicy Bypass -File \"{}\"",
            hook_script.to_string_lossy()
        )
    } else {
        let sh_script = hooks_dir.join("check-compact.sh");
        format!("bash \"{}\"", sh_script.to_string_lossy())
    };

    // 建構 hooks 設定
    let hook_entry = serde_json::json!({
        "type": "command",
        "command": hook_command
    });

    let user_prompt_submit = serde_json::json!([{
        "matcher": "",
        "hooks": [hook_entry]
    }]);

    // 合併到現有設定（保留其他 hook 設定）
    let obj = settings.as_object_mut()
        .ok_or_else(|| "Global settings.json is not an object".to_string())?;

    // 取得或建立 hooks 物件
    let hooks = obj.entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    if let Some(hooks_obj) = hooks.as_object_mut() {
        hooks_obj.insert("UserPromptSubmit".to_string(), user_prompt_submit);
    }

    // 寫回
    let output = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&settings_path, output)
        .map_err(|e| format!("Failed to write global settings.json: {}", e))?;

    eprintln!("✅ Global Claude settings configured: {}", settings_path.display());
    Ok(())
}

/// 安裝 Claude Code Skill（複製到全域位置）
#[tauri::command]
pub fn install_skill(app: tauri::AppHandle) -> Result<String, String> {
    let home = home_dir().ok_or_else(|| "Cannot find home directory".to_string())?;
    let skill_dest = home.join(".claude").join("skills").join("uni");
    let bundled_dir = get_bundled_dir(&app)?;
    let skill_source = bundled_dir.join("skill");

    if !skill_source.exists() {
        return Err(format!("Skill source not found: {:?}", skill_source));
    }

    // 建立目標目錄
    fs::create_dir_all(&skill_dest)
        .map_err(|e| format!("Failed to create skill dir: {}", e))?;

    // 複製所有檔案
    copy_dir_recursive(&skill_source, &skill_dest)?;

    // 也安裝 hooks（如果有的話）
    let hooks_source = bundled_dir.join("hooks");
    let hooks_dest = home.join(".claude").join("hooks");
    if hooks_source.exists() {
        fs::create_dir_all(&hooks_dest)
            .map_err(|e| format!("Failed to create hooks dir: {}", e))?;
        copy_dir_recursive(&hooks_source, &hooks_dest)?;
    }

    // 設定全域 ~/.claude/settings.json，讓 hook 在所有專案都生效
    if let Err(e) = configure_global_claude_settings(&home, &hooks_dest) {
        eprintln!("Warning: skill installed but failed to configure global settings: {}", e);
    }

    Ok("Claude Code Skill installed successfully".to_string())
}

/// 遞迴複製目錄
fn copy_dir_recursive(src: &PathBuf, dest: &PathBuf) -> Result<(), String> {
    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read dir: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            fs::create_dir_all(&dest_path)
                .map_err(|e| format!("Failed to create dir {:?}: {}", dest_path, e))?;
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)
                .map_err(|e| format!("Failed to copy {:?}: {}", src_path, e))?;
        }
    }
    Ok(())
}

/// 檢查是否首次啟動（setup 是否已完成）
#[tauri::command]
pub fn check_setup_done() -> bool {
    if let Some(home) = home_dir() {
        home.join(".tsunu-alive").join("setup-done.json").exists()
    } else {
        true // 找不到 home 就跳過 setup
    }
}

/// 標記 setup 完成
#[tauri::command]
pub fn mark_setup_done() -> Result<(), String> {
    let home = home_dir().ok_or_else(|| "Cannot find home directory".to_string())?;
    let tsunu_dir = home.join(".tsunu-alive");

    fs::create_dir_all(&tsunu_dir)
        .map_err(|e| format!("Failed to create dir: {}", e))?;

    let setup_file = tsunu_dir.join("setup-done.json");
    let content = serde_json::json!({
        "version": 1,
        "completedAt": chrono::Local::now().to_rfc3339(),
    });

    fs::write(&setup_file, serde_json::to_string_pretty(&content).unwrap())
        .map_err(|e| format!("Failed to write setup-done.json: {}", e))?;

    Ok(())
}
