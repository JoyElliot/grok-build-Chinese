//! In-app how-to documentation data (embedded markdown).
//!
//! Single source of truth: two static arrays (`USER_GUIDE`, `REFERENCE_DOCS`) hold every doc.
//! All lookups are zero-allocation; `DocEntry` exists only for backward compatibility with the TUI doc picker.

/// Stable document identity, independent from its localized display title.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DocId(&'static str);

impl DocId {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

pub const GETTING_STARTED: DocId = DocId::new("user-guide.getting-started");
pub const AUTHENTICATION: DocId = DocId::new("user-guide.authentication");
pub const KEYBOARD_SHORTCUTS: DocId = DocId::new("user-guide.keyboard-shortcuts");
pub const SLASH_COMMANDS: DocId = DocId::new("user-guide.slash-commands");
pub const CONFIGURATION: DocId = DocId::new("user-guide.configuration");
pub const THEMING: DocId = DocId::new("user-guide.theming");
pub const MCP_SERVERS: DocId = DocId::new("user-guide.mcp-servers");
pub const SKILLS: DocId = DocId::new("user-guide.skills");
pub const PLUGINS: DocId = DocId::new("user-guide.plugins");
pub const HOOKS: DocId = DocId::new("user-guide.hooks");
pub const CUSTOM_MODELS: DocId = DocId::new("user-guide.custom-models");
pub const PROJECT_RULES: DocId = DocId::new("user-guide.project-rules");
pub const MEMORY: DocId = DocId::new("user-guide.memory");
pub const HEADLESS_MODE: DocId = DocId::new("user-guide.headless-mode");
pub const AGENT_MODE: DocId = DocId::new("user-guide.agent-mode");
pub const SUBAGENTS: DocId = DocId::new("user-guide.subagents");
pub const SESSIONS: DocId = DocId::new("user-guide.sessions");
pub const SANDBOX: DocId = DocId::new("user-guide.sandbox");
pub const PLAN_MODE: DocId = DocId::new("user-guide.plan-mode");
pub const BACKGROUND_TASKS: DocId = DocId::new("user-guide.background-tasks");
pub const TERMINAL_SUPPORT: DocId = DocId::new("user-guide.terminal-support");
pub const PERMISSIONS: DocId = DocId::new("user-guide.permissions-and-safety");
pub const DASHBOARD: DocId = DocId::new("user-guide.dashboard");
pub const MONITORING_USAGE: DocId = DocId::new("user-guide.monitoring-usage");
pub const STATUS_LINE: DocId = DocId::new("user-guide.status-line");
pub const CONFIG_REFERENCE: DocId = DocId::new("user-guide.config-reference");
pub const GROK_CLONE: DocId = DocId::new("user-guide.grok-clone");
pub const HOOKS_AND_PLUGINS: DocId = DocId::new("reference.hooks-and-plugins");
pub const CUSTOM_HOOKS: DocId = DocId::new("reference.custom-hooks");

/// A compile-time document entry. All text fields are `&'static str`.
#[derive(Debug)]
pub struct Doc {
    pub id: DocId,
    pub filename: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub content: &'static str,
}

/// Owned variant for the TUI doc picker (backward compat).
#[derive(Debug, Clone)]
pub struct DocEntry {
    pub id: DocId,
    pub title: String,
    pub description: String,
    /// Embedded markdown content.
    pub content: &'static str,
}

impl From<&Doc> for DocEntry {
    fn from(d: &Doc) -> Self {
        Self {
            id: d.id,
            title: d.title.into(),
            description: d.description.into(),
            content: d.content,
        }
    }
}

// ── Static doc tables ────────────────────────────────────────────────────────

macro_rules! guide {
    ($id:expr, $file:literal, $title:literal, $desc:literal) => {
        Doc {
            id: $id,
            filename: $file,
            title: $title,
            description: $desc,
            content: include_str!(concat!("../docs/user-guide/", $file)),
        }
    };
}

pub static USER_GUIDE: &[Doc] = &[
    guide!(
        GETTING_STARTED,
        "01-getting-started.md",
        "Getting Started",
        "Installation, first launch, and basic interaction"
    ),
    guide!(
        AUTHENTICATION,
        "02-authentication.md",
        "Authentication",
        "Browser login, API keys, OIDC, external auth providers"
    ),
    guide!(
        KEYBOARD_SHORTCUTS,
        "03-keyboard-shortcuts.md",
        "Keyboard Shortcuts",
        "Complete reference for all TUI key bindings"
    ),
    guide!(
        SLASH_COMMANDS,
        "04-slash-commands.md",
        "Slash Commands",
        "All / commands, including goals, research, and workflow management"
    ),
    guide!(
        CONFIGURATION,
        "05-configuration.md",
        "Configuration",
        "config.toml, pager.toml, environment variables, file locations"
    ),
    guide!(
        THEMING,
        "06-theming.md",
        "Theming and Appearance",
        "Themes, color support, pager.toml customization"
    ),
    guide!(
        MCP_SERVERS,
        "07-mcp-servers.md",
        "MCP Servers",
        "Setting up external tool integrations via MCP"
    ),
    guide!(
        SKILLS,
        "08-skills.md",
        "Skills",
        "Creating and using reusable prompt packages"
    ),
    guide!(
        PLUGINS,
        "09-plugins.md",
        "Plugins and Marketplace",
        "Installing, managing, and creating plugin packages"
    ),
    guide!(
        HOOKS,
        "10-hooks.md",
        "Hooks",
        "Project lifecycle scripts for pre/post tool-use events"
    ),
    guide!(
        CUSTOM_MODELS,
        "11-custom-models.md",
        "Custom Models",
        "BYOK, Ollama, OpenAI-compatible endpoints"
    ),
    guide!(
        PROJECT_RULES,
        "12-project-rules.md",
        "Project Rules (AGENTS.md)",
        "Per-directory instructions and precedence rules"
    ),
    guide!(
        MEMORY,
        "13-memory.md",
        "Memory",
        "Cross-session knowledge persistence and search"
    ),
    guide!(
        HEADLESS_MODE,
        "14-headless-mode.md",
        "Headless Mode and Scripting",
        "Non-interactive CLI for automation and CI/CD"
    ),
    guide!(
        AGENT_MODE,
        "15-agent-mode.md",
        "Agent Mode and IDE Integration",
        "ACP stdio transport, WebSocket relay, SDK integration"
    ),
    guide!(
        SUBAGENTS,
        "16-subagents.md",
        "Subagents and Personas",
        "Spawning parallel child agents with specialized roles"
    ),
    guide!(
        SESSIONS,
        "17-sessions.md",
        "Session Management",
        "Save, load, resume, rewind, and compact sessions"
    ),
    guide!(
        SANDBOX,
        "18-sandbox.md",
        "Sandbox Mode",
        "OS-level filesystem and network isolation"
    ),
    guide!(
        PLAN_MODE,
        "19-plan-mode.md",
        "Plan Mode",
        "Structured planning with approval dialogs"
    ),
    guide!(
        BACKGROUND_TASKS,
        "20-background-tasks.md",
        "Background Tasks and Monitoring",
        "Background commands, /loop, monitor, scheduler"
    ),
    guide!(
        TERMINAL_SUPPORT,
        "21-terminal-support.md",
        "Terminal Support and Troubleshooting",
        "tmux, Byobu, Zellij, SSH, truecolor, clipboard, and diagnostics"
    ),
    guide!(
        PERMISSIONS,
        "22-permissions-and-safety.md",
        "Permissions and Safety",
        "Modes, authorization order, allow/ask/deny rules, matching, and hooks"
    ),
    guide!(
        DASHBOARD,
        "23-dashboard.md",
        "Agent Dashboard",
        "Live multi-session roster: peek, dispatch, pin, stop, and search"
    ),
    guide!(
        MONITORING_USAGE,
        "24-monitoring-usage.md",
        "Monitoring Usage (External OpenTelemetry)",
        "Export usage metrics to a customer OpenTelemetry collector"
    ),
    guide!(
        STATUS_LINE,
        "25-status-line.md",
        "Status Line",
        "A bottom row of live session context, or the output of your own script"
    ),
    guide!(
        CONFIG_REFERENCE,
        "26-config-reference.md",
        "Configuration Reference",
        "Field list for config.toml, managed_config.toml, and requirements.toml"
    ),
    // Direct include_str! so gazelle can put this file in compile_data.
    // `guide!` hides the path inside concat!($file) and gazelle cannot see it.
    Doc {
        id: GROK_CLONE,
        filename: "27-grok-clone.md",
        title: "grok clone",
        description: "Depth-1 Grove clone, --full-history, and safe deepen/switch commands",
        content: include_str!("../docs/user-guide/27-grok-clone.md"),
    },
];

/// Non-user-guide reference docs. Bundled via `include_str!` so they are available at runtime without a docs path.
static REFERENCE_DOCS: &[Doc] = &[
    Doc {
        id: HOOKS_AND_PLUGINS,
        filename: "hooks-and-plugins.md",
        title: "Hooks & Plugins Guide",
        description: "Using hooks, plugins, and marketplace",
        content: include_str!("../docs/hooks-and-plugins.md"),
    },
    Doc {
        id: CUSTOM_HOOKS,
        filename: "custom-hooks.md",
        title: "Creating Custom Hooks",
        description: "Writing your own hooks and matchers",
        content: include_str!("../docs/custom-hooks.md"),
    },
];

struct DocTranslation {
    id: DocId,
    title: &'static str,
    description: &'static str,
    /// A missing body deliberately falls back to the canonical English guide.
    content: Option<&'static str>,
}

macro_rules! zh_doc {
    ($id:expr, $title:literal, $description:literal) => {
        DocTranslation {
            id: $id,
            title: $title,
            description: $description,
            content: None,
        }
    };
    ($id:expr, $title:literal, $description:literal, $file:literal) => {
        DocTranslation {
            id: $id,
            title: $title,
            description: $description,
            content: Some(include_str!(concat!("../docs/user-guide/zh-CN/", $file))),
        }
    };
}

macro_rules! zh_reference_doc {
    ($id:expr, $title:literal, $description:literal, $file:literal) => {
        DocTranslation {
            id: $id,
            title: $title,
            description: $description,
            content: Some(include_str!(concat!("../docs/zh-CN/", $file))),
        }
    };
}

/// Chinese display metadata and long-form bodies for the complete guide
/// catalog. Stable IDs and the canonical English source table remain unchanged.
static ZH_CN_DOCS: &[DocTranslation] = &[
    zh_doc!(
        GETTING_STARTED,
        "入门指南",
        "安装、首次启动和基本交互",
        "01-getting-started.md"
    ),
    zh_doc!(
        AUTHENTICATION,
        "身份验证",
        "浏览器登录、API 密钥、OIDC 和外部身份提供方",
        "02-authentication.md"
    ),
    zh_doc!(
        KEYBOARD_SHORTCUTS,
        "键盘快捷键",
        "完整的 TUI 按键绑定参考",
        "03-keyboard-shortcuts.md"
    ),
    zh_doc!(
        SLASH_COMMANDS,
        "斜杠命令",
        "全部 / 命令，包括目标、研究和工作流管理",
        "04-slash-commands.md"
    ),
    zh_doc!(
        CONFIGURATION,
        "配置",
        "config.toml、pager.toml、环境变量和文件位置",
        "05-configuration.md"
    ),
    zh_doc!(
        THEMING,
        "主题与外观",
        "主题、颜色支持和 pager.toml 自定义",
        "06-theming.md"
    ),
    zh_doc!(
        MCP_SERVERS,
        "MCP 服务器",
        "通过 MCP 设置外部工具集成",
        "07-mcp-servers.md"
    ),
    zh_doc!(SKILLS, "技能", "创建并使用可复用的提示包", "08-skills.md"),
    zh_doc!(
        PLUGINS,
        "插件与市场",
        "安装、管理和创建插件包",
        "09-plugins.md"
    ),
    zh_doc!(
        HOOKS,
        "钩子",
        "工具使用前后事件的项目生命周期脚本",
        "10-hooks.md"
    ),
    zh_doc!(
        CUSTOM_MODELS,
        "自定义模型",
        "BYOK、Ollama 和 OpenAI 兼容端点",
        "11-custom-models.md"
    ),
    zh_doc!(
        PROJECT_RULES,
        "项目规则（AGENTS.md）",
        "按目录生效的指令与优先级规则",
        "12-project-rules.md"
    ),
    zh_doc!(MEMORY, "记忆", "跨会话知识持久化与搜索", "13-memory.md"),
    zh_doc!(
        HEADLESS_MODE,
        "无头模式与脚本",
        "用于自动化和 CI/CD 的非交互式 CLI",
        "14-headless-mode.md"
    ),
    zh_doc!(
        AGENT_MODE,
        "智能体模式与 IDE 集成",
        "ACP stdio 传输、WebSocket 中继和 SDK 集成",
        "15-agent-mode.md"
    ),
    zh_doc!(
        SUBAGENTS,
        "子智能体与角色",
        "生成具有专门角色的并行子智能体",
        "16-subagents.md"
    ),
    zh_doc!(
        SESSIONS,
        "会话管理",
        "保存、加载、恢复、回退和压缩会话",
        "17-sessions.md"
    ),
    zh_doc!(
        SANDBOX,
        "沙箱模式",
        "操作系统级文件系统与网络隔离",
        "18-sandbox.md"
    ),
    zh_doc!(
        PLAN_MODE,
        "计划模式",
        "通过审批对话框进行结构化规划",
        "19-plan-mode.md"
    ),
    zh_doc!(
        BACKGROUND_TASKS,
        "后台任务与监控",
        "后台命令、/loop、监视器和调度器",
        "20-background-tasks.md"
    ),
    zh_doc!(
        TERMINAL_SUPPORT,
        "终端支持与故障排查",
        "tmux、Byobu、Zellij、SSH、真彩色、剪贴板和诊断",
        "21-terminal-support.md"
    ),
    zh_doc!(
        PERMISSIONS,
        "权限与安全",
        "模式、授权顺序、allow/ask/deny 规则、匹配和钩子",
        "22-permissions-and-safety.md"
    ),
    zh_doc!(
        DASHBOARD,
        "智能体面板",
        "实时多会话列表：查看、派发、固定、停止和搜索",
        "23-dashboard.md"
    ),
    zh_doc!(
        MONITORING_USAGE,
        "使用量监控（外部 OpenTelemetry）",
        "将使用量指标导出到用户的 OpenTelemetry 收集器",
        "24-monitoring-usage.md"
    ),
    zh_doc!(
        STATUS_LINE,
        "状态行",
        "会话上下文实时底栏，或自定义脚本的输出",
        "25-status-line.md"
    ),
    zh_doc!(
        CONFIG_REFERENCE,
        "配置参考",
        "config.toml、managed_config.toml 和 requirements.toml 的字段列表",
        "26-config-reference.md"
    ),
    zh_doc!(
        GROK_CLONE,
        "grok clone",
        "浅层克隆 Grove 工作区、获取完整历史并安全加深或切换分支",
        "27-grok-clone.md"
    ),
    zh_reference_doc!(
        HOOKS_AND_PLUGINS,
        "钩子与插件指南",
        "使用钩子、插件和插件市场",
        "hooks-and-plugins.md"
    ),
    zh_reference_doc!(
        CUSTOM_HOOKS,
        "创建自定义钩子",
        "编写自己的钩子和匹配器",
        "custom-hooks.md"
    ),
];

// ── Public API ───────────────────────────────────────────────────────────────

/// Find a doc by canonical or localized title. Protocol-facing identities stay
/// stable because localized matches are resolved back to the canonical table.
pub fn find_doc(title: &str) -> Option<&'static Doc> {
    USER_GUIDE
        .iter()
        .chain(REFERENCE_DOCS.iter())
        .find(|d| d.title.eq_ignore_ascii_case(title))
        .or_else(|| {
            ZH_CN_DOCS
                .iter()
                .find(|translation| translation.title == title)
                .and_then(|translation| find_doc_by_id(translation.id))
        })
}

/// Find a document by its locale-independent identity.
pub fn find_doc_by_id(id: DocId) -> Option<&'static Doc> {
    USER_GUIDE
        .iter()
        .chain(REFERENCE_DOCS.iter())
        .find(|doc| doc.id == id)
}

fn zh_translation(id: DocId) -> Option<&'static DocTranslation> {
    ZH_CN_DOCS.iter().find(|translation| translation.id == id)
}

/// Resolve localized display metadata and content, with an explicit English
/// fallback when a long-form Chinese body has not landed yet.
pub fn localized_doc(id: DocId, locale: crate::locale::UiLocale) -> Option<DocEntry> {
    let doc = find_doc_by_id(id)?;
    if locale == crate::locale::UiLocale::ZhCn
        && let Some(translation) = zh_translation(id)
    {
        return Some(DocEntry {
            id,
            title: translation.title.to_owned(),
            description: translation.description.to_owned(),
            content: translation.content.unwrap_or(doc.content),
        });
    }
    Some(DocEntry::from(doc))
}

/// All doc titles, zero allocation.
pub fn all_titles() -> impl Iterator<Item = &'static str> {
    USER_GUIDE
        .iter()
        .chain(REFERENCE_DOCS.iter())
        .map(|d| d.title)
}

/// Returns the content of a how-to document by exact title match (case-insensitive).
pub fn get_howto_doc(title: &str) -> Option<&'static str> {
    find_doc(title).map(|d| d.content)
}

/// Returns a list of available how-to titles for the model to choose from.
pub fn list_howto_titles() -> Vec<String> {
    all_titles().map(String::from).collect()
}

/// Returns all docs as owned `DocEntry` values for the TUI doc picker.
pub fn default_howto_entries() -> Vec<DocEntry> {
    default_howto_entries_for(crate::locale::UiLocale::EnUs)
}

/// Returns all docs for a locale while preserving the canonical ordering and
/// stable IDs used by commands and tutorial deep-links.
pub fn default_howto_entries_for(locale: crate::locale::UiLocale) -> Vec<DocEntry> {
    USER_GUIDE
        .iter()
        .chain(REFERENCE_DOCS.iter())
        .filter_map(|doc| localized_doc(doc.id, locale))
        .collect()
}

/// Extract user-guide docs to `<grok_home>/docs/user-guide/`.
///
/// Called from the pager binary startup so the model can read them from disk.
pub fn extract_user_guide_docs(grok_home: &std::path::Path) {
    extract_user_guide_docs_for_locale(grok_home, crate::locale::UiLocale::EnUs);
}

/// Extract the selected locale to the product-owned runtime guide directory.
/// Missing translated bodies fall back one document at a time to English.
pub fn extract_user_guide_docs_for_locale(
    grok_home: &std::path::Path,
    locale: crate::locale::UiLocale,
) {
    let docs_dir = grok_home.join("docs").join("user-guide");
    if let Err(e) = std::fs::create_dir_all(&docs_dir) {
        tracing::warn!(error = %e, "Failed to create user-guide docs directory");
        return;
    }
    for doc in USER_GUIDE {
        let content = localized_doc(doc.id, locale)
            .map(|entry| entry.content)
            .unwrap_or(doc.content);
        if let Err(e) = std::fs::write(docs_dir.join(doc.filename), content) {
            tracing::debug!(error = %e, filename = doc.filename, "Failed to extract user-guide doc");
        }
    }
    // Clean up stale managed docs (files removed from USER_GUIDE since last run).
    // Only remove files matching the managed naming pattern (NN-*.md).
    if let Ok(entries) = std::fs::read_dir(&docs_dir) {
        let valid: std::collections::HashSet<&str> =
            USER_GUIDE.iter().map(|d| d.filename).collect();
        for dir_entry in entries.flatten() {
            if let Some(name) = dir_entry.file_name().to_str() {
                let bytes = name.as_bytes();
                let is_managed = name.len() > 3
                    && bytes.first().is_some_and(|b| b.is_ascii_digit())
                    && bytes.get(1).is_some_and(|b| b.is_ascii_digit())
                    && bytes.get(2) == Some(&b'-')
                    && name.ends_with(".md");
                if is_managed
                    && !valid.contains(name)
                    && let Err(e) = std::fs::remove_file(dir_entry.path())
                {
                    tracing::debug!(error = %e, filename = name, "Failed to remove stale user-guide doc");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_guide_entries_are_valid() {
        for doc in USER_GUIDE {
            assert!(!doc.content.is_empty(), "Doc {} is empty", doc.filename);
            assert!(
                !doc.title.is_empty(),
                "Doc {} has empty title",
                doc.filename
            );
            assert!(
                !doc.description.is_empty(),
                "Doc {} has empty description",
                doc.filename
            );
            assert!(
                doc.content.starts_with('#'),
                "Doc {} should start with a markdown header",
                doc.filename
            );
        }
    }

    #[test]
    fn user_guide_entries_have_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        let mut ids = std::collections::HashSet::new();
        for doc in USER_GUIDE {
            assert!(
                seen.insert(doc.filename),
                "Duplicate doc in list: {}",
                doc.filename
            );
            assert!(
                ids.insert(doc.id),
                "Duplicate document id: {}",
                doc.id.as_str()
            );
        }
    }

    #[test]
    fn chinese_catalog_covers_every_document_id() {
        let canonical: std::collections::HashSet<_> = USER_GUIDE
            .iter()
            .chain(REFERENCE_DOCS.iter())
            .map(|doc| doc.id)
            .collect();
        let translated: std::collections::HashSet<_> =
            ZH_CN_DOCS.iter().map(|doc| doc.id).collect();
        assert_eq!(translated, canonical);
    }

    #[test]
    fn zh_localization_chinese_catalog_has_a_localized_body_for_every_document() {
        for translation in ZH_CN_DOCS {
            let content = translation
                .content
                .unwrap_or_else(|| panic!("Missing Chinese body for {}", translation.id.as_str()));
            assert!(
                content.lines().any(|line| line.starts_with("# ")),
                "Chinese body for {} should contain a top-level Markdown heading",
                translation.id.as_str()
            );
            assert!(
                content
                    .chars()
                    .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)),
                "Chinese body for {} should contain Han characters",
                translation.id.as_str()
            );
            assert!(
                !content.contains('\r'),
                "Chinese body for {} contains carriage returns; built-in docs must be LF-only",
                translation.id.as_str()
            );
        }
    }

    #[test]
    fn default_howto_entries_includes_all_user_guide_docs() {
        let entries = default_howto_entries();
        assert_eq!(entries.len(), USER_GUIDE.len() + REFERENCE_DOCS.len());
        for (i, (entry, doc)) in entries.iter().zip(USER_GUIDE.iter()).enumerate() {
            assert_eq!(entry.title, doc.title, "Entry {} title mismatch", i);
        }
    }

    #[test]
    fn find_doc_is_case_insensitive() {
        let doc = find_doc("getting started").expect("should find Getting Started");
        assert_eq!(doc.title, "Getting Started");
        assert!(find_doc("nonexistent guide").is_none());
    }

    #[test]
    fn find_doc_accepts_chinese_display_title_without_changing_identity() {
        assert_eq!(
            find_doc("入门指南").map(|doc| doc.id),
            Some(GETTING_STARTED)
        );
    }

    #[test]
    fn stable_id_and_chinese_content_do_not_depend_on_english_title() {
        let canonical = find_doc_by_id(GETTING_STARTED).expect("stable id resolves");
        assert_eq!(canonical.title, "Getting Started");
        let chinese = localized_doc(GETTING_STARTED, crate::locale::UiLocale::ZhCn).unwrap();
        assert_eq!(chinese.id, GETTING_STARTED);
        assert_eq!(chinese.title, "入门指南");
        assert_ne!(chinese.content, canonical.content);
        assert!(chinese.content.starts_with("# 入门指南"));
        assert!(chinese.content.contains("社区构建说明"));

        let authentication = localized_doc(AUTHENTICATION, crate::locale::UiLocale::ZhCn).unwrap();
        assert_ne!(
            authentication.content,
            find_doc_by_id(AUTHENTICATION).unwrap().content
        );
        assert!(authentication.content.starts_with("# 身份验证"));
    }

    #[test]
    fn all_titles_covers_both_tables() {
        let titles: Vec<_> = all_titles().collect();
        assert_eq!(titles.len(), USER_GUIDE.len() + REFERENCE_DOCS.len());
    }

    #[test]
    fn get_howto_doc_delegates_to_find_doc() {
        assert!(get_howto_doc("Getting Started").is_some());
        assert!(get_howto_doc("Hooks & Plugins Guide").is_some());
        assert!(get_howto_doc("no such doc").is_none());
    }

    #[test]
    fn list_howto_titles_returns_all() {
        let titles = list_howto_titles();
        assert_eq!(titles.len(), USER_GUIDE.len() + REFERENCE_DOCS.len());
    }

    #[test]
    fn extract_writes_docs_and_cleans_stale() {
        let tmp = tempfile::tempdir().unwrap();
        let docs_dir = tmp.path().join("docs").join("user-guide");

        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(docs_dir.join("99-removed.md"), "stale").unwrap();
        std::fs::write(docs_dir.join("notes.md"), "user notes").unwrap();

        extract_user_guide_docs(tmp.path());

        for doc in USER_GUIDE {
            let path = docs_dir.join(doc.filename);
            assert!(path.exists(), "Expected doc {} to exist", doc.filename);
            let got = std::fs::read_to_string(&path).unwrap();
            assert_eq!(got, doc.content, "Content mismatch for {}", doc.filename);
        }
        assert!(
            !docs_dir.join("99-removed.md").exists(),
            "Stale doc should be cleaned up"
        );
        assert!(
            docs_dir.join("notes.md").exists(),
            "User file should not be deleted"
        );
    }

    #[test]
    fn locale_extraction_keeps_runtime_path_and_writes_translated_content() {
        let tmp = tempfile::tempdir().unwrap();
        extract_user_guide_docs_for_locale(tmp.path(), crate::locale::UiLocale::ZhCn);
        let docs_dir = tmp.path().join("docs").join("user-guide");
        for doc in USER_GUIDE {
            assert_eq!(
                std::fs::read_to_string(docs_dir.join(doc.filename)).unwrap(),
                localized_doc(doc.id, crate::locale::UiLocale::ZhCn)
                    .unwrap()
                    .content,
                "localized extraction mismatch for {}",
                doc.filename
            );
        }
        assert!(!tmp.path().join("docs").join("zh-CN").exists());
    }
}
