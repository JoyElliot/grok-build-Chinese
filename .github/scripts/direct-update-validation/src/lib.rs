//! Exact release-source tests, deliberately isolated from the full application.
#![allow(dead_code)]
// The same adapter must be compiled against four distinct exact-source modules.
#![allow(clippy::duplicate_mod)]
extern crate self as xai_grok_product;
#[path = "../../../../crates/codegen/xai-grok-product/src/lib.rs"]
mod product;
pub use product::COMMUNITY_RELEASE_REPO;
#[path = "../generated/community_update_cancel.rs"]
mod community_update_cancel;
#[path = "../generated/current/community_release.rs"]
mod current;
#[path = "../generated/v1_0_45/community_release.rs"]
mod v1_0_45;
#[path = "../generated/v1_0_8/community_release.rs"]
mod v1_0_8;
// Entry-point policy is not under test here; production release modules are
// compiled unchanged, including their internal tests and archive validators.
fn ensure_community_updates_enabled() -> anyhow::Result<()> {
    Ok(())
}
fn ensure_selected_updates_enabled() -> anyhow::Result<()> {
    Ok(())
}
fn community_updates_enabled() -> bool {
    true
}
fn updates_enabled() -> bool {
    true
}

#[path = "../generated/old_msvc/community_release.rs"]
mod old_msvc;
