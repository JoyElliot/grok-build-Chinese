// Generated case names only; each test executes unchanged source through a narrow adapter.
use serde_json::Value;

fn run_case(id: &str) {
    let matrix: Value = serde_json::from_str(include_str!("../fixtures/selection-matrix.json")).unwrap();
    assert_eq!(matrix["fixture_kind"], "SYNTHETIC_OFFLINE_API_METADATA");
    let case = matrix["cases"].as_array().unwrap().iter().find(|case| case["id"] == id).unwrap();
    let observed = match case["revision"].as_str().unwrap() {
        "v1_0_8" => super::v1_0_8::observe(case),
        "v1_0_45" => super::v1_0_45::observe(case),
        "current_msvc" => super::current_msvc::observe(case),
        other => panic!("unrecognized revision: {other}"),
    };
    let expected = &case["expect"];
    if let Some(fragment) = expected["error_contains"].as_str() {
        let error = observed.expect_err(id).to_string();
        assert!(error.contains(fragment), "{id}: expected error containing {fragment:?}, got {error:?}");
    } else {
        let observed = observed.unwrap_or_else(|error| panic!("{id}: {error:#}"));
        assert_eq!(observed.version, expected["version"].as_str().unwrap(), "{id}: version");
        assert_eq!(observed.asset_name, expected["asset_name"].as_str().unwrap(), "{id}: asset");
        if case["revision"] == "current_msvc" {
            assert_eq!(observed.target_platform.as_deref(), case["platform"].as_str(), "{id}: requesting ABI preserved");
        }
    }
}

#[test]
fn v1_0_8__frozen_six_asset_bridge__windowsx86_64gnu() { run_case("v1_0_8__frozen_six_asset_bridge__windowsx86_64gnu"); }

#[test]
fn v1_0_8__frozen_six_asset_bridge__macosaarch64() { run_case("v1_0_8__frozen_six_asset_bridge__macosaarch64"); }

#[test]
fn v1_0_8__frozen_six_asset_bridge__linuxx86_64gnu() { run_case("v1_0_8__frozen_six_asset_bridge__linuxx86_64gnu"); }

#[test]
fn v1_0_8__plain_v108_two_asset_bridge__windowsx86_64gnu() { run_case("v1_0_8__plain_v108_two_asset_bridge__windowsx86_64gnu"); }

#[test]
fn v1_0_8__plain_bridge_no_unix__macosaarch64() { run_case("v1_0_8__plain_bridge_no_unix__macosaarch64"); }

#[test]
fn v1_0_8__plain_bridge_no_unix__linuxx86_64gnu() { run_case("v1_0_8__plain_bridge_no_unix__linuxx86_64gnu"); }

#[test]
fn v1_0_8__bridge_preserved_new_native_only_skipped__windowsx86_64gnu() { run_case("v1_0_8__bridge_preserved_new_native_only_skipped__windowsx86_64gnu"); }

#[test]
fn v1_0_8__bridge_then_gnu_only_future__windowsx86_64gnu() { run_case("v1_0_8__bridge_then_gnu_only_future__windowsx86_64gnu"); }

#[test]
fn v1_0_8__bridge_then_gnu_only_future__macosaarch64() { run_case("v1_0_8__bridge_then_gnu_only_future__macosaarch64"); }

#[test]
fn v1_0_8__bridge_then_gnu_only_future__linuxx86_64gnu() { run_case("v1_0_8__bridge_then_gnu_only_future__linuxx86_64gnu"); }

#[test]
fn v1_0_8__modern_exact_six_accepted__windowsx86_64gnu() { run_case("v1_0_8__modern_exact_six_accepted__windowsx86_64gnu"); }

#[test]
fn v1_0_8__modern_exact_six_accepted__macosaarch64() { run_case("v1_0_8__modern_exact_six_accepted__macosaarch64"); }

#[test]
fn v1_0_8__modern_exact_six_accepted__linuxx86_64gnu() { run_case("v1_0_8__modern_exact_six_accepted__linuxx86_64gnu"); }

#[test]
fn v1_0_8__extra_native__windowsx86_64gnu() { run_case("v1_0_8__extra_native__windowsx86_64gnu"); }

#[test]
fn v1_0_8__missing_sidecar__windowsx86_64gnu() { run_case("v1_0_8__missing_sidecar__windowsx86_64gnu"); }

#[test]
fn v1_0_8__corrupt_unrelated__windowsx86_64gnu() { run_case("v1_0_8__corrupt_unrelated__windowsx86_64gnu"); }

#[test]
fn v1_0_8__duplicate_unrelated__windowsx86_64gnu() { run_case("v1_0_8__duplicate_unrelated__windowsx86_64gnu"); }

#[test]
fn v1_0_8__new_platform_archives__windowsx86_64gnu() { run_case("v1_0_8__new_platform_archives__windowsx86_64gnu"); }

#[test]
fn v1_0_8__selected_missing__windowsx86_64gnu() { run_case("v1_0_8__selected_missing__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_newer_skipped_missing__windowsx86_64gnu() { run_case("v1_0_8__invalid_newer_skipped_missing__windowsx86_64gnu"); }

#[test]
fn v1_0_8__selected_duplicate__windowsx86_64gnu() { run_case("v1_0_8__selected_duplicate__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_newer_skipped_duplicate__windowsx86_64gnu() { run_case("v1_0_8__invalid_newer_skipped_duplicate__windowsx86_64gnu"); }

#[test]
fn v1_0_8__selected_not_uploaded__windowsx86_64gnu() { run_case("v1_0_8__selected_not_uploaded__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_newer_skipped_not_uploaded__windowsx86_64gnu() { run_case("v1_0_8__invalid_newer_skipped_not_uploaded__windowsx86_64gnu"); }

#[test]
fn v1_0_8__selected_zero_size__windowsx86_64gnu() { run_case("v1_0_8__selected_zero_size__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_newer_skipped_zero_size__windowsx86_64gnu() { run_case("v1_0_8__invalid_newer_skipped_zero_size__windowsx86_64gnu"); }

#[test]
fn v1_0_8__selected_over_limit__windowsx86_64gnu() { run_case("v1_0_8__selected_over_limit__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_newer_skipped_over_limit__windowsx86_64gnu() { run_case("v1_0_8__invalid_newer_skipped_over_limit__windowsx86_64gnu"); }

#[test]
fn v1_0_8__selected_wrong_repository__windowsx86_64gnu() { run_case("v1_0_8__selected_wrong_repository__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_newer_skipped_wrong_repository__windowsx86_64gnu() { run_case("v1_0_8__invalid_newer_skipped_wrong_repository__windowsx86_64gnu"); }

#[test]
fn v1_0_8__selected_missing_digest__windowsx86_64gnu() { run_case("v1_0_8__selected_missing_digest__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_newer_skipped_missing_digest__windowsx86_64gnu() { run_case("v1_0_8__invalid_newer_skipped_missing_digest__windowsx86_64gnu"); }

#[test]
fn v1_0_8__selected_bad_digest__windowsx86_64gnu() { run_case("v1_0_8__selected_bad_digest__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_newer_skipped_bad_digest__windowsx86_64gnu() { run_case("v1_0_8__invalid_newer_skipped_bad_digest__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_release_skipped_draft__windowsx86_64gnu() { run_case("v1_0_8__invalid_release_skipped_draft__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_release_skipped_mutable__windowsx86_64gnu() { run_case("v1_0_8__invalid_release_skipped_mutable__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_release_skipped_wrong_namespace__windowsx86_64gnu() { run_case("v1_0_8__invalid_release_skipped_wrong_namespace__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_release_skipped_prerelease_mismatch__windowsx86_64gnu() { run_case("v1_0_8__invalid_release_skipped_prerelease_mismatch__windowsx86_64gnu"); }

#[test]
fn v1_0_8__invalid_release_skipped_build_metadata__windowsx86_64gnu() { run_case("v1_0_8__invalid_release_skipped_build_metadata__windowsx86_64gnu"); }

#[test]
fn v1_0_8__stable_excludes_prerelease__windowsx86_64gnu() { run_case("v1_0_8__stable_excludes_prerelease__windowsx86_64gnu"); }

#[test]
fn v1_0_8__alpha_includes_prerelease__windowsx86_64gnu() { run_case("v1_0_8__alpha_includes_prerelease__windowsx86_64gnu"); }

#[test]
fn v1_0_8__bad_channel__windowsx86_64gnu() { run_case("v1_0_8__bad_channel__windowsx86_64gnu"); }

#[test]
fn v1_0_8__no_releases__windowsx86_64gnu() { run_case("v1_0_8__no_releases__windowsx86_64gnu"); }

#[test]
fn v1_0_8__requested_version_mismatch__windowsx86_64gnu() { run_case("v1_0_8__requested_version_mismatch__windowsx86_64gnu"); }

#[test]
fn v1_0_45__frozen_six_asset_bridge__windowsx86_64gnu() { run_case("v1_0_45__frozen_six_asset_bridge__windowsx86_64gnu"); }

#[test]
fn v1_0_45__frozen_six_asset_bridge__macosaarch64() { run_case("v1_0_45__frozen_six_asset_bridge__macosaarch64"); }

#[test]
fn v1_0_45__frozen_six_asset_bridge__linuxx86_64gnu() { run_case("v1_0_45__frozen_six_asset_bridge__linuxx86_64gnu"); }

#[test]
fn v1_0_45__plain_v108_two_asset_bridge__windowsx86_64gnu() { run_case("v1_0_45__plain_v108_two_asset_bridge__windowsx86_64gnu"); }

#[test]
fn v1_0_45__plain_bridge_no_unix__macosaarch64() { run_case("v1_0_45__plain_bridge_no_unix__macosaarch64"); }

#[test]
fn v1_0_45__plain_bridge_no_unix__linuxx86_64gnu() { run_case("v1_0_45__plain_bridge_no_unix__linuxx86_64gnu"); }

#[test]
fn v1_0_45__bridge_preserved_new_native_only_skipped__windowsx86_64gnu() { run_case("v1_0_45__bridge_preserved_new_native_only_skipped__windowsx86_64gnu"); }

#[test]
fn v1_0_45__bridge_then_gnu_only_future__windowsx86_64gnu() { run_case("v1_0_45__bridge_then_gnu_only_future__windowsx86_64gnu"); }

#[test]
fn v1_0_45__bridge_then_gnu_only_future__macosaarch64() { run_case("v1_0_45__bridge_then_gnu_only_future__macosaarch64"); }

#[test]
fn v1_0_45__bridge_then_gnu_only_future__linuxx86_64gnu() { run_case("v1_0_45__bridge_then_gnu_only_future__linuxx86_64gnu"); }

#[test]
fn v1_0_45__modern_exact_six_accepted__windowsx86_64gnu() { run_case("v1_0_45__modern_exact_six_accepted__windowsx86_64gnu"); }

#[test]
fn v1_0_45__modern_exact_six_accepted__macosaarch64() { run_case("v1_0_45__modern_exact_six_accepted__macosaarch64"); }

#[test]
fn v1_0_45__modern_exact_six_accepted__linuxx86_64gnu() { run_case("v1_0_45__modern_exact_six_accepted__linuxx86_64gnu"); }

#[test]
fn v1_0_45__extra_native__windowsx86_64gnu() { run_case("v1_0_45__extra_native__windowsx86_64gnu"); }

#[test]
fn v1_0_45__missing_sidecar__windowsx86_64gnu() { run_case("v1_0_45__missing_sidecar__windowsx86_64gnu"); }

#[test]
fn v1_0_45__corrupt_unrelated__windowsx86_64gnu() { run_case("v1_0_45__corrupt_unrelated__windowsx86_64gnu"); }

#[test]
fn v1_0_45__duplicate_unrelated__windowsx86_64gnu() { run_case("v1_0_45__duplicate_unrelated__windowsx86_64gnu"); }

#[test]
fn v1_0_45__new_platform_archives__windowsx86_64gnu() { run_case("v1_0_45__new_platform_archives__windowsx86_64gnu"); }

#[test]
fn v1_0_45__selected_missing__windowsx86_64gnu() { run_case("v1_0_45__selected_missing__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_newer_skipped_missing__windowsx86_64gnu() { run_case("v1_0_45__invalid_newer_skipped_missing__windowsx86_64gnu"); }

#[test]
fn v1_0_45__selected_duplicate__windowsx86_64gnu() { run_case("v1_0_45__selected_duplicate__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_newer_skipped_duplicate__windowsx86_64gnu() { run_case("v1_0_45__invalid_newer_skipped_duplicate__windowsx86_64gnu"); }

#[test]
fn v1_0_45__selected_not_uploaded__windowsx86_64gnu() { run_case("v1_0_45__selected_not_uploaded__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_newer_skipped_not_uploaded__windowsx86_64gnu() { run_case("v1_0_45__invalid_newer_skipped_not_uploaded__windowsx86_64gnu"); }

#[test]
fn v1_0_45__selected_zero_size__windowsx86_64gnu() { run_case("v1_0_45__selected_zero_size__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_newer_skipped_zero_size__windowsx86_64gnu() { run_case("v1_0_45__invalid_newer_skipped_zero_size__windowsx86_64gnu"); }

#[test]
fn v1_0_45__selected_over_limit__windowsx86_64gnu() { run_case("v1_0_45__selected_over_limit__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_newer_skipped_over_limit__windowsx86_64gnu() { run_case("v1_0_45__invalid_newer_skipped_over_limit__windowsx86_64gnu"); }

#[test]
fn v1_0_45__selected_wrong_repository__windowsx86_64gnu() { run_case("v1_0_45__selected_wrong_repository__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_newer_skipped_wrong_repository__windowsx86_64gnu() { run_case("v1_0_45__invalid_newer_skipped_wrong_repository__windowsx86_64gnu"); }

#[test]
fn v1_0_45__selected_missing_digest__windowsx86_64gnu() { run_case("v1_0_45__selected_missing_digest__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_newer_skipped_missing_digest__windowsx86_64gnu() { run_case("v1_0_45__invalid_newer_skipped_missing_digest__windowsx86_64gnu"); }

#[test]
fn v1_0_45__selected_bad_digest__windowsx86_64gnu() { run_case("v1_0_45__selected_bad_digest__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_newer_skipped_bad_digest__windowsx86_64gnu() { run_case("v1_0_45__invalid_newer_skipped_bad_digest__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_release_skipped_draft__windowsx86_64gnu() { run_case("v1_0_45__invalid_release_skipped_draft__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_release_skipped_mutable__windowsx86_64gnu() { run_case("v1_0_45__invalid_release_skipped_mutable__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_release_skipped_wrong_namespace__windowsx86_64gnu() { run_case("v1_0_45__invalid_release_skipped_wrong_namespace__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_release_skipped_prerelease_mismatch__windowsx86_64gnu() { run_case("v1_0_45__invalid_release_skipped_prerelease_mismatch__windowsx86_64gnu"); }

#[test]
fn v1_0_45__invalid_release_skipped_build_metadata__windowsx86_64gnu() { run_case("v1_0_45__invalid_release_skipped_build_metadata__windowsx86_64gnu"); }

#[test]
fn v1_0_45__stable_excludes_prerelease__windowsx86_64gnu() { run_case("v1_0_45__stable_excludes_prerelease__windowsx86_64gnu"); }

#[test]
fn v1_0_45__alpha_includes_prerelease__windowsx86_64gnu() { run_case("v1_0_45__alpha_includes_prerelease__windowsx86_64gnu"); }

#[test]
fn v1_0_45__bad_channel__windowsx86_64gnu() { run_case("v1_0_45__bad_channel__windowsx86_64gnu"); }

#[test]
fn v1_0_45__no_releases__windowsx86_64gnu() { run_case("v1_0_45__no_releases__windowsx86_64gnu"); }

#[test]
fn v1_0_45__requested_version_mismatch__windowsx86_64gnu() { run_case("v1_0_45__requested_version_mismatch__windowsx86_64gnu"); }

#[test]
fn current_msvc__frozen_six_asset_bridge__windowsx86_64gnu() { run_case("current_msvc__frozen_six_asset_bridge__windowsx86_64gnu"); }

#[test]
fn current_msvc__frozen_six_asset_bridge__macosaarch64() { run_case("current_msvc__frozen_six_asset_bridge__macosaarch64"); }

#[test]
fn current_msvc__frozen_six_asset_bridge__linuxx86_64gnu() { run_case("current_msvc__frozen_six_asset_bridge__linuxx86_64gnu"); }

#[test]
fn current_msvc__plain_v108_two_asset_bridge__windowsx86_64gnu() { run_case("current_msvc__plain_v108_two_asset_bridge__windowsx86_64gnu"); }

#[test]
fn current_msvc__plain_bridge_no_unix__macosaarch64() { run_case("current_msvc__plain_bridge_no_unix__macosaarch64"); }

#[test]
fn current_msvc__plain_bridge_no_unix__linuxx86_64gnu() { run_case("current_msvc__plain_bridge_no_unix__linuxx86_64gnu"); }

#[test]
fn current_msvc__bridge_preserved_new_native_only_skipped__windowsx86_64gnu() { run_case("current_msvc__bridge_preserved_new_native_only_skipped__windowsx86_64gnu"); }

#[test]
fn current_msvc__bridge_then_gnu_only_future__windowsx86_64gnu() { run_case("current_msvc__bridge_then_gnu_only_future__windowsx86_64gnu"); }

#[test]
fn current_msvc__bridge_then_gnu_only_future__macosaarch64() { run_case("current_msvc__bridge_then_gnu_only_future__macosaarch64"); }

#[test]
fn current_msvc__bridge_then_gnu_only_future__linuxx86_64gnu() { run_case("current_msvc__bridge_then_gnu_only_future__linuxx86_64gnu"); }

#[test]
fn current_msvc__modern_exact_six_accepted__windowsx86_64gnu() { run_case("current_msvc__modern_exact_six_accepted__windowsx86_64gnu"); }

#[test]
fn current_msvc__modern_exact_six_accepted__macosaarch64() { run_case("current_msvc__modern_exact_six_accepted__macosaarch64"); }

#[test]
fn current_msvc__modern_exact_six_accepted__linuxx86_64gnu() { run_case("current_msvc__modern_exact_six_accepted__linuxx86_64gnu"); }

#[test]
fn current_msvc__extra_native__windowsx86_64gnu() { run_case("current_msvc__extra_native__windowsx86_64gnu"); }

#[test]
fn current_msvc__missing_sidecar__windowsx86_64gnu() { run_case("current_msvc__missing_sidecar__windowsx86_64gnu"); }

#[test]
fn current_msvc__corrupt_unrelated__windowsx86_64gnu() { run_case("current_msvc__corrupt_unrelated__windowsx86_64gnu"); }

#[test]
fn current_msvc__duplicate_unrelated__windowsx86_64gnu() { run_case("current_msvc__duplicate_unrelated__windowsx86_64gnu"); }

#[test]
fn current_msvc__new_platform_archives__windowsx86_64gnu() { run_case("current_msvc__new_platform_archives__windowsx86_64gnu"); }

#[test]
fn current_msvc__selected_missing__windowsx86_64gnu() { run_case("current_msvc__selected_missing__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_newer_skipped_missing__windowsx86_64gnu() { run_case("current_msvc__invalid_newer_skipped_missing__windowsx86_64gnu"); }

#[test]
fn current_msvc__selected_duplicate__windowsx86_64gnu() { run_case("current_msvc__selected_duplicate__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_newer_skipped_duplicate__windowsx86_64gnu() { run_case("current_msvc__invalid_newer_skipped_duplicate__windowsx86_64gnu"); }

#[test]
fn current_msvc__selected_not_uploaded__windowsx86_64gnu() { run_case("current_msvc__selected_not_uploaded__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_newer_skipped_not_uploaded__windowsx86_64gnu() { run_case("current_msvc__invalid_newer_skipped_not_uploaded__windowsx86_64gnu"); }

#[test]
fn current_msvc__selected_zero_size__windowsx86_64gnu() { run_case("current_msvc__selected_zero_size__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_newer_skipped_zero_size__windowsx86_64gnu() { run_case("current_msvc__invalid_newer_skipped_zero_size__windowsx86_64gnu"); }

#[test]
fn current_msvc__selected_over_limit__windowsx86_64gnu() { run_case("current_msvc__selected_over_limit__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_newer_skipped_over_limit__windowsx86_64gnu() { run_case("current_msvc__invalid_newer_skipped_over_limit__windowsx86_64gnu"); }

#[test]
fn current_msvc__selected_wrong_repository__windowsx86_64gnu() { run_case("current_msvc__selected_wrong_repository__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_newer_skipped_wrong_repository__windowsx86_64gnu() { run_case("current_msvc__invalid_newer_skipped_wrong_repository__windowsx86_64gnu"); }

#[test]
fn current_msvc__selected_missing_digest__windowsx86_64gnu() { run_case("current_msvc__selected_missing_digest__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_newer_skipped_missing_digest__windowsx86_64gnu() { run_case("current_msvc__invalid_newer_skipped_missing_digest__windowsx86_64gnu"); }

#[test]
fn current_msvc__selected_bad_digest__windowsx86_64gnu() { run_case("current_msvc__selected_bad_digest__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_newer_skipped_bad_digest__windowsx86_64gnu() { run_case("current_msvc__invalid_newer_skipped_bad_digest__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_release_skipped_draft__windowsx86_64gnu() { run_case("current_msvc__invalid_release_skipped_draft__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_release_skipped_mutable__windowsx86_64gnu() { run_case("current_msvc__invalid_release_skipped_mutable__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_release_skipped_wrong_namespace__windowsx86_64gnu() { run_case("current_msvc__invalid_release_skipped_wrong_namespace__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_release_skipped_prerelease_mismatch__windowsx86_64gnu() { run_case("current_msvc__invalid_release_skipped_prerelease_mismatch__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_release_skipped_build_metadata__windowsx86_64gnu() { run_case("current_msvc__invalid_release_skipped_build_metadata__windowsx86_64gnu"); }

#[test]
fn current_msvc__stable_excludes_prerelease__windowsx86_64gnu() { run_case("current_msvc__stable_excludes_prerelease__windowsx86_64gnu"); }

#[test]
fn current_msvc__alpha_includes_prerelease__windowsx86_64gnu() { run_case("current_msvc__alpha_includes_prerelease__windowsx86_64gnu"); }

#[test]
fn current_msvc__bad_channel__windowsx86_64gnu() { run_case("current_msvc__bad_channel__windowsx86_64gnu"); }

#[test]
fn current_msvc__no_releases__windowsx86_64gnu() { run_case("current_msvc__no_releases__windowsx86_64gnu"); }

#[test]
fn current_msvc__requested_version_mismatch__windowsx86_64gnu() { run_case("current_msvc__requested_version_mismatch__windowsx86_64gnu"); }

#[test]
fn current_msvc__native_only_successor__windowsx86_64msvc() { run_case("current_msvc__native_only_successor__windowsx86_64msvc"); }

#[test]
fn current_msvc__gnu_only_potential_fallback__windowsx86_64msvc() { run_case("current_msvc__gnu_only_potential_fallback__windowsx86_64msvc"); }

#[test]
fn current_msvc__native_prefers_same_version__windowsx86_64msvc() { run_case("current_msvc__native_prefers_same_version__windowsx86_64msvc"); }

#[test]
fn current_msvc__newer_gnu_beats_older_native__windowsx86_64msvc() { run_case("current_msvc__newer_gnu_beats_older_native__windowsx86_64msvc"); }

#[test]
fn current_msvc__native_nplus1_after_gnu_n__windowsx86_64msvc() { run_case("current_msvc__native_nplus1_after_gnu_n__windowsx86_64msvc"); }

#[test]
fn current_msvc__invalid_native_fails_closed_duplicate__windowsx86_64msvc() { run_case("current_msvc__invalid_native_fails_closed_duplicate__windowsx86_64msvc"); }

#[test]
fn current_msvc__gnu_ignores_unrelated_invalid_native_duplicate__windowsx86_64gnu() { run_case("current_msvc__gnu_ignores_unrelated_invalid_native_duplicate__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_native_fails_closed_not_uploaded__windowsx86_64msvc() { run_case("current_msvc__invalid_native_fails_closed_not_uploaded__windowsx86_64msvc"); }

#[test]
fn current_msvc__gnu_ignores_unrelated_invalid_native_not_uploaded__windowsx86_64gnu() { run_case("current_msvc__gnu_ignores_unrelated_invalid_native_not_uploaded__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_native_fails_closed_zero_size__windowsx86_64msvc() { run_case("current_msvc__invalid_native_fails_closed_zero_size__windowsx86_64msvc"); }

#[test]
fn current_msvc__gnu_ignores_unrelated_invalid_native_zero_size__windowsx86_64gnu() { run_case("current_msvc__gnu_ignores_unrelated_invalid_native_zero_size__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_native_fails_closed_over_limit__windowsx86_64msvc() { run_case("current_msvc__invalid_native_fails_closed_over_limit__windowsx86_64msvc"); }

#[test]
fn current_msvc__gnu_ignores_unrelated_invalid_native_over_limit__windowsx86_64gnu() { run_case("current_msvc__gnu_ignores_unrelated_invalid_native_over_limit__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_native_fails_closed_wrong_repository__windowsx86_64msvc() { run_case("current_msvc__invalid_native_fails_closed_wrong_repository__windowsx86_64msvc"); }

#[test]
fn current_msvc__gnu_ignores_unrelated_invalid_native_wrong_repository__windowsx86_64gnu() { run_case("current_msvc__gnu_ignores_unrelated_invalid_native_wrong_repository__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_native_fails_closed_missing_digest__windowsx86_64msvc() { run_case("current_msvc__invalid_native_fails_closed_missing_digest__windowsx86_64msvc"); }

#[test]
fn current_msvc__gnu_ignores_unrelated_invalid_native_missing_digest__windowsx86_64gnu() { run_case("current_msvc__gnu_ignores_unrelated_invalid_native_missing_digest__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_native_fails_closed_bad_digest__windowsx86_64msvc() { run_case("current_msvc__invalid_native_fails_closed_bad_digest__windowsx86_64msvc"); }

#[test]
fn current_msvc__gnu_ignores_unrelated_invalid_native_bad_digest__windowsx86_64gnu() { run_case("current_msvc__gnu_ignores_unrelated_invalid_native_bad_digest__windowsx86_64gnu"); }

#[test]
fn current_msvc__invalid_newer_gnu_fails_closed__windowsx86_64msvc() { run_case("current_msvc__invalid_newer_gnu_fails_closed__windowsx86_64msvc"); }

#[test]
fn current_msvc__valid_native_ignores_invalid_gnu__windowsx86_64msvc() { run_case("current_msvc__valid_native_ignores_invalid_gnu__windowsx86_64msvc"); }

#[test]
fn current_msvc__per_platform_asset__windowsx86_64gnu() { run_case("current_msvc__per_platform_asset__windowsx86_64gnu"); }

#[test]
fn current_msvc__native_x64_not_other_platform_fallback__windowsx86_64gnu() { run_case("current_msvc__native_x64_not_other_platform_fallback__windowsx86_64gnu"); }

#[test]
fn current_msvc__per_platform_asset__windowsx86_64msvc() { run_case("current_msvc__per_platform_asset__windowsx86_64msvc"); }

#[test]
fn current_msvc__per_platform_asset__windowsaarch64msvc() { run_case("current_msvc__per_platform_asset__windowsaarch64msvc"); }

#[test]
fn current_msvc__native_x64_not_other_platform_fallback__windowsaarch64msvc() { run_case("current_msvc__native_x64_not_other_platform_fallback__windowsaarch64msvc"); }

#[test]
fn current_msvc__per_platform_asset__macosaarch64() { run_case("current_msvc__per_platform_asset__macosaarch64"); }

#[test]
fn current_msvc__native_x64_not_other_platform_fallback__macosaarch64() { run_case("current_msvc__native_x64_not_other_platform_fallback__macosaarch64"); }

#[test]
fn current_msvc__per_platform_asset__macosx86_64() { run_case("current_msvc__per_platform_asset__macosx86_64"); }

#[test]
fn current_msvc__native_x64_not_other_platform_fallback__macosx86_64() { run_case("current_msvc__native_x64_not_other_platform_fallback__macosx86_64"); }

#[test]
fn current_msvc__per_platform_asset__linuxx86_64gnu() { run_case("current_msvc__per_platform_asset__linuxx86_64gnu"); }

#[test]
fn current_msvc__native_x64_not_other_platform_fallback__linuxx86_64gnu() { run_case("current_msvc__native_x64_not_other_platform_fallback__linuxx86_64gnu"); }

#[test]
fn current_msvc__per_platform_asset__linuxaarch64gnu() { run_case("current_msvc__per_platform_asset__linuxaarch64gnu"); }

#[test]
fn current_msvc__native_x64_not_other_platform_fallback__linuxaarch64gnu() { run_case("current_msvc__native_x64_not_other_platform_fallback__linuxaarch64gnu"); }

#[test]
fn current_msvc__msvc_stable_excludes_prerelease__windowsx86_64msvc() { run_case("current_msvc__msvc_stable_excludes_prerelease__windowsx86_64msvc"); }

#[test]
fn current_msvc__msvc_alpha_includes_prerelease__windowsx86_64msvc() { run_case("current_msvc__msvc_alpha_includes_prerelease__windowsx86_64msvc"); }

#[test]
fn v1_0_8__asset_order_irrelevant__windowsx86_64gnu() { run_case("v1_0_8__asset_order_irrelevant__windowsx86_64gnu"); }

#[test]
fn v1_0_45__asset_order_irrelevant__windowsx86_64gnu() { run_case("v1_0_45__asset_order_irrelevant__windowsx86_64gnu"); }

#[test]
fn current_msvc__asset_order_irrelevant__windowsx86_64gnu() { run_case("current_msvc__asset_order_irrelevant__windowsx86_64gnu"); }
