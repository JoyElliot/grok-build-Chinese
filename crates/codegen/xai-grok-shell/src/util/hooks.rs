use std::path::Path;

use xai_grok_hooks::discovery::DiscoveryOptions;
use xai_grok_hooks::error::HookError;
use xai_grok_hooks::trust::DisabledHooks;
use xai_grok_hooks::trust::Trust;
use xai_grok_workspace::hook_inputs::ProcessHookInputs;
use xai_grok_workspace::permission::resolution::managed_settings;

pub(crate) fn process_hook_inputs() -> ProcessHookInputs {
    ProcessHookInputs::read(crate::claude_import::import_marker())
}

/// For a session that dispatches the hooks it discovers.
pub(crate) fn session_hook_inputs() -> (ProcessHookInputs, DisabledHooks) {
    ProcessHookInputs::read_with_disabled(managed_settings(), crate::claude_import::import_marker())
}

/// Every session startup and mid-session reload loads hooks through this function.
/// This is the one place that chooses which hook sources to load.
pub(crate) fn discover_hooks(
    inputs: &ProcessHookInputs,
    git_root: Option<&Path>,
    compat: &xai_grok_tools::types::compat::CompatConfig,
    trust: Trust,
) -> (xai_grok_hooks::discovery::HookRegistry, Vec<HookError>) {
    xai_grok_hooks::discovery::assemble_hooks(
        inputs.config_layers(),
        DiscoveryOptions {
            git_root,
            grok_home: inputs.grok_home(),
            home: inputs.home(),
            compat: compat.hooks(),
            claude_import: inputs.claude_import(),
            trust,
        },
    )
}
