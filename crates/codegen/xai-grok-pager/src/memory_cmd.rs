use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use xai_grok_shell::session::memory::storage::MemoryStorage;

use crate::locale::LocaleContext;

#[derive(Debug, clap::Args, Clone)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Debug, Subcommand, Clone)]
pub enum MemoryCommand {
    /// 清除记忆文件（默认为工作区范围）
    Clear {
        /// 清除工作区记忆（MEMORY.md、sessions/、index.sqlite）
        #[arg(long, group = "scope")]
        workspace: bool,
        /// 清除全局 MEMORY.md
        #[arg(long, group = "scope")]
        global: bool,
        /// 同时清除工作区和全局记忆
        #[arg(long, group = "scope")]
        all: bool,
        /// 跳过确认提示
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

struct ClearTarget {
    label_key: &'static str,
    label: &'static str,
    path: PathBuf,
    clear: fn(&MemoryStorage) -> std::io::Result<bool>,
}

fn workspace_target(storage: &MemoryStorage) -> ClearTarget {
    ClearTarget {
        label_key: "memory.cli.workspace_label",
        label: "workspace memory",
        path: storage.workspace_dir().to_path_buf(),
        clear: |s| s.clear_workspace(),
    }
}

fn global_target(storage: &MemoryStorage) -> ClearTarget {
    ClearTarget {
        label_key: "memory.cli.global_label",
        label: if storage.mode().is_v2() {
            "global memory"
        } else {
            "global MEMORY.md"
        },
        path: if storage.mode().is_v2() {
            storage.global_dir().to_path_buf()
        } else {
            storage.global_memory_file()
        },
        clear: |s| s.clear_global(),
    }
}

pub fn run(args: MemoryArgs, mode: xai_grok_shell::config::MemoryMode) -> Result<()> {
    run_with_locale(args, mode, &LocaleContext::default())
}

pub fn run_with_locale(
    args: MemoryArgs,
    mode: xai_grok_shell::config::MemoryMode,
    locale: &LocaleContext,
) -> Result<()> {
    match args.command {
        MemoryCommand::Clear {
            global, all, yes, ..
        } => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
            let storage = MemoryStorage::new_for_mode(&cwd, None, mode);

            let targets = if all {
                vec![workspace_target(&storage), global_target(&storage)]
            } else if global {
                vec![global_target(&storage)]
            } else {
                vec![workspace_target(&storage)]
            };

            run_clear(&storage, &targets, yes, locale)
        }
    }
}

fn run_clear(
    storage: &MemoryStorage,
    targets: &[ClearTarget],
    skip_confirm: bool,
    locale: &LocaleContext,
) -> Result<()> {
    let existing: Vec<_> = targets.iter().filter(|t| t.path.exists()).collect();

    if existing.is_empty() {
        println!(
            "{}",
            locale.named_text(
                "memory.cli.nothing_to_clear",
                "Nothing to clear: no memory files found."
            )
        );
        return Ok(());
    }

    println!(
        "{}",
        locale.named_text("memory.cli.will_delete", "The following will be deleted:")
    );
    for t in &existing {
        println!(
            "  {}: {}",
            locale.named_text(t.label_key, t.label),
            t.path.display()
        );
    }

    if !skip_confirm {
        print!(
            "\n{}",
            locale.named_text("memory.cli.confirm", "Are you sure? [y/N] ")
        );
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            println!(
                "{}",
                locale.named_text("memory.cli.cancelled", "Cancelled.")
            );
            return Ok(());
        }
    }

    let mut cleared = false;
    let mut errors: Vec<String> = Vec::new();
    for t in targets {
        match (t.clear)(storage) {
            Ok(true) => {
                cleared = true;
                println!(
                    "  {}",
                    locale
                        .named_text("memory.cli.cleared_item", "Cleared: {label}")
                        .replace("{label}", &locale.named_text(t.label_key, t.label))
                );
            }
            Ok(false) => {} // nothing to clear for this scope
            Err(e) => {
                errors.push(format!("{}: {e}", locale.named_text(t.label_key, t.label)));
            }
        }
    }

    if cleared && errors.is_empty() {
        println!(
            "{}",
            locale.named_text("memory.cli.cleared", "Memory cleared.")
        );
    } else if cleared {
        println!(
            "{}",
            locale.named_text(
                "memory.cli.partially_cleared",
                "Memory partially cleared. Errors:"
            )
        );
        for e in &errors {
            eprintln!("  {e}");
        }
    } else if !errors.is_empty() {
        eprintln!(
            "{}",
            locale.named_text("memory.cli.failed", "Failed to clear memory:")
        );
        for e in &errors {
            eprintln!("  {e}");
        }
        return Err(anyhow::anyhow!("clear failed"));
    }

    Ok(())
}
