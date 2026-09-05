//! Display-only localization overlays for recognized product catalog entries.
//!
//! The structs passed to this module remain the canonical search, routing, and
//! action payloads. Every overlay is fail-closed at the presentation boundary:
//! the available source marker, canonical identity, and complete current English
//! value must all match before a translated string is returned.

use super::{LocaleContext, extension_text};
use crate::views::managed_mcp_localization::{
    KNOWN_MANAGED_MCP_TOOLS, KnownManagedMcpTool, known_managed_mcp_tool,
    localized_managed_mcp_tool_description, localized_managed_mcp_tool_label,
    managed_connector_display_name,
};
use crate::views::mcps_modal::{McpServerInfo, McpToolDetail, McpWireSource};
use xai_grok_tools::implementations::skills::types::{SkillInfo, SkillScope};
use xai_grok_tools::util::grok_home;
use xai_hooks_plugins_types::{MarketplacePluginEntry, MarketplaceScanResult};

struct ExactCopy {
    canonical: &'static str,
    english: &'static str,
    key: &'static str,
}

const OFFICIAL_MARKETPLACE_ENTRIES: &[ExactCopy] = &[
    ExactCopy {
        canonical: "vercel",
        english: "Vercel deployment platform integration. Manage deployments, check build status, access logs, configure domains, and control your frontend infrastructure directly from Grok.",
        key: "extensions.catalog.marketplace.vercel.description",
    },
    ExactCopy {
        canonical: "sentry",
        english: "Sentry error monitoring integration. Access error reports, analyze stack traces, search issues by fingerprint, and debug production errors directly from your development environment.",
        key: "extensions.catalog.marketplace.sentry.description",
    },
    ExactCopy {
        canonical: "chrome-devtools",
        english: "Chrome DevTools integration. Control and inspect a live Chrome browser. Record performance traces, analyze network requests, check console messages with source-mapped stack traces, and automate browser actions.",
        key: "extensions.catalog.marketplace.chrome_devtools.description",
    },
    ExactCopy {
        canonical: "cloudflare",
        english: "Skills for the Cloudflare developer platform: Workers, Durable Objects, Agents SDK, MCP servers, Wrangler CLI, and web performance.",
        key: "extensions.catalog.marketplace.cloudflare.description",
    },
    ExactCopy {
        canonical: "superpowers",
        english: "Core skills library for software development: test-driven development, systematic debugging, collaboration patterns, and proven engineering workflows and techniques.",
        key: "extensions.catalog.marketplace.superpowers.description",
    },
    ExactCopy {
        canonical: "base44",
        english: "Build and deploy Base44 full-stack apps with CLI project management and JavaScript/TypeScript SDK development skills.",
        key: "extensions.catalog.marketplace.base44.description",
    },
    ExactCopy {
        canonical: "mongodb",
        english: "Official plugin for MongoDB (Self-Managed MCP Server + Skills). Connect to any MongoDB deployment (Community, Enterprise Advanced, local dev container via Atlas CLI, or Atlas clusters) through a self-managed MongoDB MCP Server using your connection string. Explore data, manage collections, optimize queries, and generate reliable code with MongoDB best practices.",
        key: "extensions.catalog.marketplace.mongodb.description",
    },
    ExactCopy {
        canonical: "mongodb-atlas",
        english: "Official plugin for MongoDB Atlas (Managed MCP Server + Skills). Sign in with your Atlas account to explore data, manage collections, optimize queries, generate reliable code with MongoDB best practices, and manage Atlas resources such as clusters, projects, database users, and network access.",
        key: "extensions.catalog.marketplace.mongodb_atlas.description",
    },
    ExactCopy {
        canonical: "axiom",
        english: "Official Axiom observability integration (MCP Server + Skills). Query logs and metrics with APL, run hypothesis-driven SRE investigations, build dashboards, manage monitors and alerts, translate Splunk SPL queries, and analyze and optimize costs.",
        key: "extensions.catalog.marketplace.axiom.description",
    },
    ExactCopy {
        canonical: "neon",
        english: "Neon Serverless Postgres integration (MCP Server + Skills). Get started with Neon, manage projects and databases, pick connection methods, and create branches for migration testing and isolated development.",
        key: "extensions.catalog.marketplace.neon.description",
    },
    ExactCopy {
        canonical: "wix",
        english: "Build, manage, and deploy Wix sites and apps (MCP Server + Skills). Includes CLI development skills and Wix MCP server for site management, eCommerce, CMS, dashboard extensions, and more.",
        key: "extensions.catalog.marketplace.wix.description",
    },
    ExactCopy {
        canonical: "netlify",
        english: "Skills for the Netlify platform: serverless and edge functions, Blobs storage, managed Postgres, Image CDN, forms, identity, caching, AI Gateway, framework adapters, and the Netlify CLI and deploys.",
        key: "extensions.catalog.marketplace.netlify.description",
    },
    ExactCopy {
        canonical: "firecrawl",
        english: "Turn any website into clean, LLM-ready markdown or structured data. Search, scrape, map, crawl, and extract live web data via the bundled hosted Firecrawl MCP server — keyless on eligible networks (1,000 free credits/month, no signup), with an optional free API key for more usage. Automatic JS rendering, anti-bot handling, and proxy rotation; CLI skills available as a fallback.",
        key: "extensions.catalog.marketplace.firecrawl.description",
    },
    ExactCopy {
        canonical: "figma",
        english: "Official Figma MCP server and skills for design-to-code workflows. Read design context from Figma files, implement designs, use Code Connect, write to the canvas, and generate Figma designs from web pages.",
        key: "extensions.catalog.marketplace.figma.description",
    },
    ExactCopy {
        canonical: "exa",
        english: "Exa is the fastest and most accurate web search for AI. It searches the web in real time, reads relevant pages, and answers with up-to-date sources. Use it to find the latest code docs, news, company information, and much more. Use the exa-search skill for deep research, and sign up for Exa to get free credits.",
        key: "extensions.catalog.marketplace.exa.description",
    },
    ExactCopy {
        canonical: "tavily",
        english: "Web search, content extraction, website crawling, URL discovery, and deep research via Tavily's hosted MCP server and official skills. Connect with OAuth to access current, source-grounded web information directly from Grok Build.",
        key: "extensions.catalog.marketplace.tavily.description",
    },
    ExactCopy {
        canonical: "railway",
        english: "Railway deployment platform integration (MCP server + skill). Create projects, provision services and databases, deploy code, manage environments, variables, volumes, object storage buckets, and feature flags, configure domains, troubleshoot build failures, and check status and metrics directly from Grok.",
        key: "extensions.catalog.marketplace.railway.description",
    },
    ExactCopy {
        canonical: "stripe",
        english: "Stripe development plugin for Grok Build: best practices, API/SDK upgrade guidance, and the Stripe MCP server.",
        key: "extensions.catalog.marketplace.stripe.description",
    },
    ExactCopy {
        canonical: "tinyfish",
        english: "Web search, content extraction, and goal-driven browser automation via TinyFish's hosted MCP server. Search and fetch pages for free, then drive real multi-step workflows on live sites — including authenticated ones, using saved Browser Context Profiles and password-manager credentials. Sign in with your TinyFish account on first connection; no API key needed.",
        key: "extensions.catalog.marketplace.tinyfish.description",
    },
    ExactCopy {
        canonical: "pstack",
        english: "pstack (poteto-mode): rigorous agent playbooks and principles for writing less, higher-quality code. Investigation, design, review, verification, and parallel subagent workflows.",
        key: "extensions.catalog.marketplace.pstack.description",
    },
    ExactCopy {
        canonical: "browser-use",
        english: "Give Grok a real browser — the user's own Chrome, with their logins, or an isolated Browser Use Cloud browser. Use it whenever a task involves a website or web app: browsing, scraping and data extraction, filling forms, testing sites, taking screenshots, automating web workflows. Runs locally via uvx; no API key needed for local Chrome.",
        key: "extensions.catalog.marketplace.browser_use.description",
    },
    ExactCopy {
        canonical: "quo",
        english: "Official Quo MCP for Grok Build. Send individual, group, or bulk SMS; manage contacts and tasks; review message history and call transcripts; and follow up on missed calls through Quo's hosted OAuth MCP server.",
        key: "extensions.catalog.marketplace.quo.description",
    },
];

// The indexed official catalog supplies one Neon description, while the
// marketplace-local plugin manifest supplies another. The scanner intentionally
// gives the manifest precedence, so accept both exact, official copies at the
// display boundary without changing the canonical marketplace metadata.
const OFFICIAL_MARKETPLACE_DESCRIPTION_ALIASES: &[ExactCopy] = &[ExactCopy {
    canonical: "neon",
    english: "Manage your Neon Serverless Postgres projects, databases, and branches with the Neon agent skills and the Neon MCP Server.",
    key: "extensions.catalog.marketplace.neon.manifest_description",
}];

const BUNDLED_SKILL_ENTRIES: &[ExactCopy] = &[
    ExactCopy {
        canonical: "build-with-ai",
        english: "Build AI apps on SpaceXAI (XAI_API_KEY + api.x.ai)",
        key: "slash.command.build-with-ai.description",
    },
    ExactCopy {
        canonical: "code-review",
        english: "Run an extremely strict maintainability review for abstraction quality, giant files, and spaghetti-condition growth. Use for a deep code quality audit or an especially harsh maintainability review.",
        key: "slash.command.code-review.description",
    },
    ExactCopy {
        canonical: "create-skill",
        english: "Create a new Grok skill",
        key: "slash.command.create-skill.description",
    },
    ExactCopy {
        canonical: "create-workflow",
        english: "Author a new multi-agent workflow",
        key: "slash.command.create-workflow.description",
    },
    ExactCopy {
        canonical: "design",
        english: "Run the full design-doc-writer and design-doc-reviewer loop until consensus. Produces a polished design document with a PR plan.",
        key: "slash.command.design.description",
    },
    ExactCopy {
        canonical: "docx",
        english: "Use this skill whenever the user wants to create, read, edit, or manipulate Word documents (.docx or .dotx files). Triggers include any mention of 'Word doc', 'word document', '.docx', '.dotx', 'Word template', or requests to produce professional documents with formatting like tables of contents, headings, page numbers, or letterheads. Also use when extracting or reorganizing content from .docx/.dotx files, inserting or replacing images in documents, performing find-and-replace in Word files, working with tracked changes or comments, or converting content into a polished Word document. If the user asks for a 'report', 'memo', 'letter', 'template', 'ticket', 'card', or similar deliverable as a Word or .docx file, use this skill. Do NOT use for PDFs, spreadsheets, Google Docs, or general coding tasks unrelated to document generation.",
        key: "extensions.catalog.skill.docx.description",
    },
    ExactCopy {
        canonical: "execute-plan",
        english: "Execute a PR Plan DAG from a design document. Parses the plan, topologically sorts it, implements PRs in parallel using worktree-isolated subagents, runs mandatory orchestrator-level review, and assembles either a Graphite PR stack or a plain-git branch stack depending on tool availability.",
        key: "slash.command.execute-plan.description",
    },
    ExactCopy {
        canonical: "game-animation-frames",
        english: "Video-first animation frames that actually cycle",
        key: "extensions.catalog.skill.game_animation_frames.description",
    },
    ExactCopy {
        canonical: "game-asset-core",
        english: "Core rules + engine-ready defaults for game assets",
        key: "extensions.catalog.skill.game_asset_core.description",
    },
    ExactCopy {
        canonical: "game-character-consistency",
        english: "Same character, every image",
        key: "extensions.catalog.skill.game_character_consistency.description",
    },
    ExactCopy {
        canonical: "game-tilesets",
        english: "Seamless tiles and transition sets that actually tile",
        key: "extensions.catalog.skill.game_tilesets.description",
    },
    ExactCopy {
        canonical: "game-ui-icons",
        english: "Game UI kits and icon sets",
        key: "extensions.catalog.skill.game_ui_icons.description",
    },
    ExactCopy {
        canonical: "imagine",
        english: "Prompting and workflow guidance for Imagine image tools",
        key: "slash.command.bundled:imagine.description",
    },
    ExactCopy {
        canonical: "implement",
        english: "Run the full implement-review-fix loop using implementer and reviewer personas. Supports effort-based multi-reviewer scaling (1-5 reviewers) with automatic specialization selection. Includes memory-based feedback loop that learns from past review patterns. Loops until all reviewers find 0 issues of any severity.",
        key: "slash.command.implement.description",
    },
    ExactCopy {
        canonical: "pdf",
        english: "Read, create, and transform PDF files. Covers pulling text and tables out of PDFs, generating new PDFs, merging and splitting documents, rotating pages, watermarking, encrypting or removing passwords, extracting embedded images, running OCR on scanned documents, and filling out PDF forms including official tax forms. Apply this skill whenever a task involves a .pdf file as input or deliverable.",
        key: "extensions.catalog.skill.pdf.description",
    },
    ExactCopy {
        canonical: "pptx",
        english: "Use this skill any time a .pptx file is involved in any way — as input, output, or both. This includes creating slide decks, pitch decks, or presentations; reading, parsing, or extracting text from any .pptx file (even if the extracted content will be used elsewhere, like in an email or summary); editing, modifying, or updating existing presentations; combining or splitting slide files; working with templates, layouts, speaker notes, or comments. Trigger whenever the user mentions 'deck', 'slides', 'presentation', or references a .pptx filename, regardless of what they plan to do with the content afterward. If a .pptx file needs to be opened, created, or touched, use this skill.",
        key: "extensions.catalog.skill.pptx.description",
    },
    ExactCopy {
        canonical: "pr-babysit",
        english: "Monitor PRs, fix CI failures, address review comments, resolve merge conflicts, and restack stacks. Supports independent PRs, Graphite stacks, and GitHub stacked PRs (gh-stack).",
        key: "slash.command.pr-babysit.description",
    },
    ExactCopy {
        canonical: "resume-claude",
        english: "Continue from a recent Claude Code session",
        key: "slash.command.resume-claude.description",
    },
    ExactCopy {
        canonical: "resume-codex",
        english: "Continue from a recent Codex session",
        key: "slash.command.resume-codex.description",
    },
    ExactCopy {
        canonical: "resume-cursor",
        english: "Continue from a recent Cursor session",
        key: "slash.command.resume-cursor.description",
    },
    ExactCopy {
        canonical: "review",
        english: "Run a reviewer subagent against uncommitted local changes, a named branch, or a GitHub PR. Local and branch modes write a review file plus a summary to disk. PR mode posts the findings as a PENDING GitHub review for the user to inspect and submit through the UI.",
        key: "slash.command.review.description",
    },
    ExactCopy {
        canonical: "skill-design-principles",
        english: "Concise, high-signal principles for writing and editing skills well. Use whenever authoring or editing a skill.",
        key: "extensions.catalog.skill.skill_design_principles.description",
    },
    ExactCopy {
        canonical: "long-running-background-tasks",
        english: "Required reading before you start, watch, or wait on anything that keeps running after you launch it — background jobs, watchers, scheduled loops, CI, pull requests, training runs, dev servers, long builds. Read it before you launch such work and before you report on its state. Saying where a running job stands counts as working on it. Use when: about to launch, supervise, inspect, diagnose, or report on work that keeps running after it is started.",
        key: "extensions.catalog.skill.long_running_background_tasks.description",
    },
    ExactCopy {
        canonical: "statusline",
        english: "Configure the Grok Build status line.",
        key: "extensions.catalog.skill.statusline.description",
    },
];

fn exact_entry<'a>(
    entries: &'a [ExactCopy],
    canonical: &str,
    english: &str,
) -> Option<&'a ExactCopy> {
    entries
        .iter()
        .find(|entry| entry.canonical == canonical && entry.english == english)
}

/// Stronger than the legacy official-first sorting hint: localization only
/// trusts the exact Git source registered by the client.
pub(super) fn is_trusted_official_marketplace_source(source: &MarketplaceScanResult) -> bool {
    source.source_kind == "git"
        && source.source_url_or_path == xai_grok_plugin_marketplace::OFFICIAL_SOURCE_GIT_URL
}

pub(super) fn localized_marketplace_source_label(
    source: &MarketplaceScanResult,
    locale: Option<&LocaleContext>,
) -> String {
    if is_trusted_official_marketplace_source(source) && source.source_name == "xAI Official" {
        extension_text(
            locale,
            "extensions.catalog.marketplace.official_source",
            &source.source_name,
        )
    } else {
        source.source_name.clone()
    }
}

fn official_marketplace_category(name: &str) -> Option<&'static str> {
    match name {
        "vercel" | "netlify" | "railway" => Some("deployment"),
        "sentry" => Some("monitoring"),
        "mongodb" | "mongodb-atlas" | "neon" => Some("database"),
        "axiom" => Some("observability"),
        "quo" => Some("productivity"),
        "chrome-devtools" | "cloudflare" | "superpowers" | "base44" | "wix" | "firecrawl"
        | "figma" | "exa" | "tavily" | "stripe" | "tinyfish" | "pstack" | "browser-use" => {
            Some("development")
        }
        _ => None,
    }
}

fn trusted_marketplace_entry(
    source: &MarketplaceScanResult,
    plugin: &MarketplacePluginEntry,
) -> Option<&'static ExactCopy> {
    if !is_trusted_official_marketplace_source(source) {
        return None;
    }
    let expected_category = official_marketplace_category(&plugin.name)?;
    if plugin.category.as_deref() != Some(expected_category) {
        return None;
    }
    let expected_relative_path = if plugin.name == "neon" {
        "external_plugins/neon"
    } else {
        plugin.name.as_str()
    };
    if plugin.relative_path != expected_relative_path {
        return None;
    }
    let description = plugin.description.as_deref()?;
    exact_entry(OFFICIAL_MARKETPLACE_ENTRIES, &plugin.name, description).or_else(|| {
        exact_entry(
            OFFICIAL_MARKETPLACE_DESCRIPTION_ALIASES,
            &plugin.name,
            description,
        )
    })
}

pub(super) fn localized_marketplace_description(
    source: &MarketplaceScanResult,
    plugin: &MarketplacePluginEntry,
    locale: Option<&LocaleContext>,
) -> String {
    let raw = plugin.description.as_deref().unwrap_or("");
    trusted_marketplace_entry(source, plugin)
        .map(|entry| extension_text(locale, entry.key, raw))
        .unwrap_or_else(|| raw.to_owned())
}

pub(super) fn localized_marketplace_category(
    source: &MarketplaceScanResult,
    plugin: &MarketplacePluginEntry,
    locale: Option<&LocaleContext>,
) -> Option<String> {
    let category = plugin.category.as_deref()?;
    if trusted_marketplace_entry(source, plugin).is_none() {
        return Some(category.to_owned());
    }
    let key = match category {
        "deployment" => "extensions.catalog.marketplace.category.deployment",
        "monitoring" => "extensions.catalog.marketplace.category.monitoring",
        "development" => "extensions.catalog.marketplace.category.development",
        "database" => "extensions.catalog.marketplace.category.database",
        "observability" => "extensions.catalog.marketplace.category.observability",
        "productivity" => "extensions.catalog.marketplace.category.productivity",
        _ => return Some(category.to_owned()),
    };
    Some(extension_text(locale, key, category))
}

fn trusted_bundled_skill(skill: &SkillInfo, raw: &str) -> Option<&'static ExactCopy> {
    if skill.scope != SkillScope::Bundled
        || skill.display_name.is_some()
        || skill.plugin_name.is_some()
        || skill.plugin_version.is_some()
        || skill.plugin_root.is_some()
        || skill.plugin_data.is_some()
        || skill.config_source.is_some()
        || !skill.has_user_specified_description
    {
        return None;
    }
    let normalized_path = skill.path.replace('\\', "/");
    let expected_path = grok_home()
        .join("bundled")
        .join("skills")
        .join(&skill.name)
        .join("SKILL.md")
        .to_string_lossy()
        .replace('\\', "/");
    if normalized_path != expected_path {
        return None;
    }
    exact_entry(BUNDLED_SKILL_ENTRIES, &skill.name, raw)
}

pub(super) fn localized_bundled_skill_description(
    skill: &SkillInfo,
    locale: Option<&LocaleContext>,
) -> String {
    let raw = skill
        .short_description
        .as_deref()
        .unwrap_or(&skill.description);
    trusted_bundled_skill(skill, raw)
        .map(|entry| extension_text(locale, entry.key, raw))
        .unwrap_or_else(|| raw.to_owned())
}

fn trusted_managed_connector(server: &McpServerInfo) -> Option<&'static str> {
    if !server.is_managed_gateway
        || server.wire_source != McpWireSource::Managed
        || server.source != "managed"
        || server.plugin_name.is_some()
    {
        return None;
    }
    let connector = match server.name.strip_prefix("managed_gateway:")? {
        "github" => "github",
        "gmail" => "gmail",
        "outlook" => "outlook",
        "tasks" => "tasks",
        _ => return None,
    };
    (server.display_name.as_deref() == managed_connector_display_name(connector))
        .then_some(connector)
}

pub(super) fn localized_mcp_server_label(
    server: &McpServerInfo,
    locale: Option<&LocaleContext>,
) -> String {
    let raw = server.display_name.as_deref().unwrap_or(&server.name);
    let Some(connector) = trusted_managed_connector(server) else {
        return raw.to_owned();
    };
    if connector != "tasks" || raw != "Automations" {
        return raw.to_owned();
    }
    let localized = extension_text(locale, "extensions.catalog.mcp.tasks.server", raw);
    if localized == raw {
        raw.to_owned()
    } else {
        format!("{localized}（{raw}）")
    }
}

fn trusted_managed_tool(
    server: &McpServerInfo,
    tool: &McpToolDetail,
) -> Option<&'static KnownManagedMcpTool> {
    let connector = trusted_managed_connector(server)?;
    let display = tool.display_name.as_deref()?;
    known_managed_mcp_tool(connector, &tool.name, display)
}

pub(super) fn localized_mcp_tool_label(
    server: &McpServerInfo,
    tool: &McpToolDetail,
    locale: Option<&LocaleContext>,
) -> String {
    let raw = tool.display_name.as_deref().unwrap_or(&tool.name);
    let Some(entry) = trusted_managed_tool(server, tool) else {
        return raw.to_owned();
    };
    let Some(localized) = locale.and_then(|locale| localized_managed_mcp_tool_label(entry, locale))
    else {
        return raw.to_owned();
    };
    format!("{localized}（{raw}）")
}

pub(super) fn localized_mcp_tool_description(
    server: &McpServerInfo,
    tool: &McpToolDetail,
    locale: Option<&LocaleContext>,
) -> String {
    let raw = tool.description.as_deref().unwrap_or("");
    let Some(entry) = trusted_managed_tool(server, tool) else {
        return raw.to_owned();
    };
    locale
        .and_then(|locale| localized_managed_mcp_tool_description(entry, raw, locale))
        .unwrap_or_else(|| raw.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::{LocaleContext, LocaleSource, ResolvedLocale, UiLocale};
    use crate::views::extensions_modal::{StatusFilter, build_mcp_servers_picker_rows_with_locale};
    use crate::views::mcps_modal::McpServerDisplayStatus;
    use std::collections::HashSet;

    fn zh() -> LocaleContext {
        LocaleContext::new(ResolvedLocale {
            locale: UiLocale::ZhCn,
            source: LocaleSource::Cli,
        })
    }

    fn official_source() -> MarketplaceScanResult {
        MarketplaceScanResult {
            source_name: "xAI Official".into(),
            source_kind: "git".into(),
            source_url_or_path: xai_grok_plugin_marketplace::OFFICIAL_SOURCE_GIT_URL.into(),
            plugins: vec![],
            error: None,
        }
    }

    fn marketplace_plugin(name: &str, description: &str, category: &str) -> MarketplacePluginEntry {
        MarketplacePluginEntry {
            name: name.into(),
            version: None,
            description: Some(description.into()),
            category: Some(category.into()),
            author: None,
            tags: vec![],
            keywords: vec![],
            domains: vec![],
            homepage: None,
            relative_path: if name == "neon" {
                "external_plugins/neon".into()
            } else {
                name.into()
            },
            skill_count: 0,
            has_hooks: false,
            has_agents: false,
            has_mcp: false,
            install_status: "not_installed".into(),
            installed_version: None,
            components: None,
            remote_url: None,
            remote_ref: None,
            remote_sha: None,
            remote_subdir: None,
        }
    }

    #[test]
    fn zh_localization_marketplace_overlay_requires_exact_official_tuple() {
        let locale = zh();
        let source = official_source();
        let entry = &OFFICIAL_MARKETPLACE_ENTRIES[0];
        let plugin = marketplace_plugin(entry.canonical, entry.english, "deployment");
        assert_eq!(
            localized_marketplace_source_label(&source, Some(&locale)),
            "xAI 官方"
        );
        assert!(
            localized_marketplace_description(&source, &plugin, Some(&locale))
                .starts_with("Vercel 部署平台集成")
        );
        assert_eq!(
            localized_marketplace_category(&source, &plugin, Some(&locale)).as_deref(),
            Some("部署")
        );

        let mut spoofed = official_source();
        spoofed.source_url_or_path = "https://example.invalid/plugin-marketplace.git".into();
        assert_eq!(
            localized_marketplace_source_label(&spoofed, Some(&locale)),
            "xAI Official"
        );
        assert_eq!(
            localized_marketplace_description(&spoofed, &plugin, Some(&locale)),
            entry.english
        );

        let drifted = marketplace_plugin(entry.canonical, "Updated upstream copy", "deployment");
        assert_eq!(
            localized_marketplace_description(&source, &drifted, Some(&locale)),
            "Updated upstream copy"
        );
        assert_eq!(
            localized_marketplace_category(&source, &drifted, Some(&locale)).as_deref(),
            Some("deployment")
        );

        let wrong_category = marketplace_plugin(entry.canonical, entry.english, "monitoring");
        assert_eq!(
            localized_marketplace_description(&source, &wrong_category, Some(&locale)),
            entry.english
        );
        assert_eq!(
            localized_marketplace_category(&source, &wrong_category, Some(&locale)).as_deref(),
            Some("monitoring")
        );

        let mut wrong_path = plugin.clone();
        wrong_path.relative_path = format!("plugins/{}/nested", entry.canonical);
        assert_eq!(
            localized_marketplace_description(&source, &wrong_path, Some(&locale)),
            entry.english
        );
        assert_eq!(
            localized_marketplace_category(&source, &wrong_path, Some(&locale)).as_deref(),
            Some("deployment")
        );

        let neon_manifest = &OFFICIAL_MARKETPLACE_DESCRIPTION_ALIASES[0];
        let neon = marketplace_plugin(neon_manifest.canonical, neon_manifest.english, "database");
        assert!(
            localized_marketplace_description(&source, &neon, Some(&locale))
                .starts_with("使用 Neon Agent 技能")
        );
        assert_eq!(
            localized_marketplace_category(&source, &neon, Some(&locale)).as_deref(),
            Some("数据库")
        );
        let changed_neon = marketplace_plugin(
            neon_manifest.canonical,
            "Manage different Neon resources.",
            "database",
        );
        assert_eq!(
            localized_marketplace_description(&source, &changed_neon, Some(&locale)),
            "Manage different Neon resources."
        );
    }

    #[test]
    fn zh_localization_all_current_official_marketplace_descriptions_are_mapped() {
        let locale = zh();
        let source = official_source();
        assert_eq!(OFFICIAL_MARKETPLACE_ENTRIES.len(), 22);
        for entry in OFFICIAL_MARKETPLACE_ENTRIES {
            let category = official_marketplace_category(entry.canonical).unwrap();
            let plugin = marketplace_plugin(entry.canonical, entry.english, category);
            let localized = localized_marketplace_description(&source, &plugin, Some(&locale));
            assert_ne!(
                localized, entry.english,
                "missing key for {}",
                entry.canonical
            );
            assert_eq!(plugin.name, entry.canonical);
            assert_eq!(plugin.description.as_deref(), Some(entry.english));
            assert_eq!(
                plugin.relative_path,
                if entry.canonical == "neon" {
                    "external_plugins/neon"
                } else {
                    entry.canonical
                }
            );
        }
    }

    #[test]
    fn zh_localization_bundled_skill_overlay_preserves_dynamic_rows() {
        let locale = zh();
        let entry = BUNDLED_SKILL_ENTRIES
            .iter()
            .find(|entry| entry.canonical == "create-skill")
            .unwrap();
        let skill = SkillInfo {
            name: entry.canonical.into(),
            description: "long description not rendered".into(),
            short_description: Some(entry.english.into()),
            has_user_specified_description: true,
            path: grok_home()
                .join("bundled")
                .join("skills")
                .join(entry.canonical)
                .join("SKILL.md")
                .to_string_lossy()
                .into_owned(),
            scope: SkillScope::Bundled,
            ..SkillInfo::default()
        };
        assert_eq!(
            localized_bundled_skill_description(&skill, Some(&locale)),
            "创建新的 Grok 技能"
        );
        assert_eq!(skill.name, "create-skill");
        assert_eq!(skill.short_description.as_deref(), Some(entry.english));

        let mut user = skill.clone();
        user.scope = SkillScope::User;
        assert_eq!(
            localized_bundled_skill_description(&user, Some(&locale)),
            entry.english
        );
        let mut drifted = skill.clone();
        drifted.short_description = Some("Create a changed Grok skill".into());
        assert_eq!(
            localized_bundled_skill_description(&drifted, Some(&locale)),
            "Create a changed Grok skill"
        );
        let mut injected = skill.clone();
        injected.path = format!("C:\\tmp\\bundled\\skills\\{}\\SKILL.md", entry.canonical);
        assert_eq!(
            localized_bundled_skill_description(&injected, Some(&locale)),
            entry.english
        );
        let mut configured = skill.clone();
        configured.config_source = Some(
            xai_grok_tools::types::config_source::ConfigSource::ConfigToml {
                path: configured.path.clone().into(),
            },
        );
        assert_eq!(
            localized_bundled_skill_description(&configured, Some(&locale)),
            entry.english
        );
        let mut stamped = skill.clone();
        stamped.config_source = Some(
            xai_grok_tools::types::config_source::ConfigSource::Bundled {
                path: stamped.path.clone().into(),
            },
        );
        assert_eq!(
            localized_bundled_skill_description(&stamped, Some(&locale)),
            entry.english
        );
    }

    #[test]
    fn zh_localization_all_current_bundled_skill_descriptions_are_mapped() {
        let locale = zh();
        assert_eq!(BUNDLED_SKILL_ENTRIES.len(), 24);
        for entry in BUNDLED_SKILL_ENTRIES {
            let skill = SkillInfo {
                name: entry.canonical.into(),
                description: entry.english.into(),
                has_user_specified_description: true,
                path: grok_home()
                    .join("bundled")
                    .join("skills")
                    .join(entry.canonical)
                    .join("SKILL.md")
                    .to_string_lossy()
                    .into_owned(),
                scope: SkillScope::Bundled,
                ..SkillInfo::default()
            };
            let localized = localized_bundled_skill_description(&skill, Some(&locale));
            assert_ne!(
                localized, entry.english,
                "missing key for {}",
                entry.canonical
            );
            assert_eq!(skill.name, entry.canonical);
            assert_eq!(skill.description, entry.english);
        }

        let pptx = BUNDLED_SKILL_ENTRIES
            .iter()
            .find(|entry| entry.canonical == "pptx")
            .unwrap();
        let stale = SkillInfo {
            name: pptx.canonical.into(),
            description: pptx
                .english
                .strip_suffix(
                    " If a .pptx file needs to be opened, created, or touched, use this skill.",
                )
                .unwrap()
                .into(),
            has_user_specified_description: true,
            path: grok_home()
                .join("bundled")
                .join("skills")
                .join(pptx.canonical)
                .join("SKILL.md")
                .to_string_lossy()
                .into_owned(),
            scope: SkillScope::Bundled,
            ..SkillInfo::default()
        };
        assert_eq!(
            localized_bundled_skill_description(&stale, Some(&locale)),
            stale.description,
            "a stale upstream description must fail closed instead of receiving a mismatched copy"
        );
    }

    fn managed_server(connector: &str, display_name: &str) -> McpServerInfo {
        McpServerInfo {
            name: format!("managed_gateway:{connector}"),
            display_name: Some(display_name.into()),
            status: McpServerDisplayStatus::Ready,
            tool_count: 1,
            auth_required: false,
            setup_required: false,
            setup: None,
            setup_values: Default::default(),
            tools: vec![],
            enabled: true,
            source: "managed".into(),
            wire_source: McpWireSource::Managed,
            plugin_name: None,
            is_managed_gateway: true,
        }
    }

    fn tasks_server() -> McpServerInfo {
        managed_server("tasks", "Automations")
    }

    #[test]
    fn zh_localization_managed_tasks_overlay_keeps_canonical_ids() {
        let locale = zh();
        let server = tasks_server();
        let entry = known_managed_mcp_tool("tasks", "tasks__create", "Create").unwrap();
        let tool = McpToolDetail {
            name: entry.qualified_name.into(),
            display_name: Some(entry.display_name.into()),
            description: None,
            enabled: true,
        };
        assert_eq!(
            localized_mcp_server_label(&server, Some(&locale)),
            "自动化（Automations）"
        );
        assert_eq!(
            localized_mcp_tool_label(&server, &tool, Some(&locale)),
            "创建自动化（Create）"
        );
        assert_eq!(server.name, "managed_gateway:tasks");
        assert_eq!(tool.name, "tasks__create");

        let mut local = tasks_server();
        local.wire_source = McpWireSource::Local;
        assert_eq!(
            localized_mcp_server_label(&local, Some(&locale)),
            "Automations"
        );
        assert_eq!(
            localized_mcp_tool_label(&local, &tool, Some(&locale)),
            "Create"
        );

        let mut spoofed = tasks_server();
        spoofed.source = "plugin: tasks".into();
        assert_eq!(
            localized_mcp_server_label(&spoofed, Some(&locale)),
            "Automations"
        );
        assert_eq!(
            localized_mcp_tool_label(&spoofed, &tool, Some(&locale)),
            "Create"
        );

        let changed = McpToolDetail {
            description: Some("Updated server description".into()),
            ..tool.clone()
        };
        assert_eq!(
            localized_mcp_tool_description(&server, &changed, Some(&locale)),
            "Updated server description"
        );

        let github = managed_server("github", "GitHub");
        let gist = McpToolDetail {
            name: "github__create_gist".into(),
            display_name: Some("Create Gist".into()),
            description: Some("Create a new gist".into()),
            enabled: true,
        };
        assert!(localized_mcp_tool_label(&github, &gist, Some(&locale)).contains("创建 Gist"));
        assert_eq!(
            localized_mcp_tool_description(&github, &gist, Some(&locale)),
            "创建新的 Gist"
        );
    }

    #[test]
    fn zh_localization_all_managed_tool_labels_are_mapped_without_rewriting_ids() {
        let locale = zh();
        assert_eq!(KNOWN_MANAGED_MCP_TOOLS.len(), 137);
        for entry in KNOWN_MANAGED_MCP_TOOLS {
            let display_name = managed_connector_display_name(entry.connector).unwrap();
            let server = managed_server(entry.connector, display_name);
            let tool = McpToolDetail {
                name: entry.qualified_name.into(),
                display_name: Some(entry.display_name.into()),
                description: None,
                enabled: true,
            };
            let localized = localized_mcp_tool_label(&server, &tool, Some(&locale));
            assert_ne!(
                localized, entry.display_name,
                "missing key for {}",
                entry.qualified_name
            );
            assert!(localized.ends_with(&format!("（{}）", entry.display_name)));
            assert_eq!(tool.name, entry.qualified_name);
        }
    }

    #[test]
    fn zh_localization_mcp_picker_preserves_routing_metadata() {
        let locale = zh();
        let mut server = tasks_server();
        server.tools = vec![McpToolDetail {
            name: "tasks__create".into(),
            display_name: Some("Create".into()),
            description: None,
            enabled: true,
        }];
        let servers = [server];
        let collapsed = HashSet::new();
        let expanded = HashSet::from([0]);
        let raw = build_mcp_servers_picker_rows_with_locale(
            &servers,
            "",
            StatusFilter::All,
            &collapsed,
            &expanded,
            None,
        );
        let localized = build_mcp_servers_picker_rows_with_locale(
            &servers,
            "",
            StatusFilter::All,
            &collapsed,
            &expanded,
            Some(&locale),
        );

        assert_eq!(localized.data_indices, raw.data_indices);
        assert_eq!(localized.group_keys, raw.group_keys);
        assert!(raw.labels.iter().any(|label| label == "Automations"));
        assert!(raw.labels.iter().any(|label| label == "Create"));
        assert!(
            localized
                .labels
                .iter()
                .any(|label| label == "自动化（Automations）")
        );
        assert!(
            localized
                .labels
                .iter()
                .any(|label| label == "创建自动化（Create）")
        );
        assert_eq!(servers[0].name, "managed_gateway:tasks");
        assert_eq!(servers[0].tools[0].name, "tasks__create");
    }
}
