//! Offline exact-source metadata selection harness. No production updater code is edited.
#![allow(dead_code)]
mod v1_0_8;
mod v1_0_45;
mod current_msvc;

#[derive(Debug)]
pub struct Observation {
    pub version: String,
    pub asset_name: String,
    pub target_platform: Option<String>,
}

#[cfg(test)]
mod matrix_tests;
