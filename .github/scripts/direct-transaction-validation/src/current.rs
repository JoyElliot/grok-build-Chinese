//! Test executable, not an installable product or an unmodified historical binary.
#![allow(dead_code)]
extern crate self as xai_grok_product;
#[path = "../generated/current/product.rs"] mod product;
pub use product::*;
#[path = "../generated/current/community_update_cancel.rs"] mod community_update_cancel;
#[path = "../generated/current/community_release.rs"] mod community_release;
fn ensure_community_updates_enabled() -> anyhow::Result<()> { Ok(()) }
fn ensure_selected_updates_enabled() -> anyhow::Result<()> { Ok(()) }
fn community_updates_enabled() -> bool { true }
fn updates_enabled() -> bool { true }
fn main() { eprintln!("Run the compiled Rust test executable through the isolated acceptance runner."); std::process::exit(2); }
