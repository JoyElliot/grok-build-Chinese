use super::*;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn display_document(domain: Domain, version: u64, entries: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": 1, "version": version, "locale": "zh-CN",
        "domain": domain, "entries": entries,
    }))
    .unwrap()
}

#[test]
fn display_bundles_parse_and_contract_rejects_nulls_crlf_and_mcp_digest_drift() {
    for domain in Domain::ALL {
        let bundled = TranslationCatalog::bundled_display(domain);
        assert_eq!(bundled.version(), 1, "invalid bundle for {domain:?}");
        assert_eq!(bundled.domain(), Some(domain));
    }
    let entry = json!({"field":"description", "context":["grok-future"], "source":"New copy", "translation":"新文案"});
    let good = display_document(Domain::Models, 2, json!([entry]));
    let value: serde_json::Value = serde_json::from_slice(&good).unwrap();
    for (key, invalid) in [
        ("source_sha256", serde_json::Value::Null),
        ("source", serde_json::Value::Null),
    ] {
        let mut bad = value.clone();
        bad["entries"][0][key] = invalid;
        let raw = serde_json::to_vec(&bad).unwrap();
        assert!(TranslationCatalog::parse(manifest(2, &raw), &raw).is_err());
    }
    let mut bad = value.clone();
    bad["domain"] = serde_json::Value::Null;
    let raw = serde_json::to_vec(&bad).unwrap();
    assert!(TranslationCatalog::parse(manifest(2, &raw), &raw).is_err());
    let raw = serde_json::to_string_pretty(&value)
        .unwrap()
        .replace('\n', "\r\n")
        .into_bytes();
    assert!(TranslationCatalog::parse(manifest(2, &raw), &raw).is_err());
    let raw = display_document(
        Domain::Mcp,
        2,
        json!([{
            "field":"tool_description", "context":["future", "search", "0".repeat(64)],
            "source":"New copy", "translation":"新文案"
        }]),
    );
    assert!(TranslationCatalog::parse(manifest(2, &raw), &raw).is_err());
}

#[tokio::test]
async fn display_domains_reject_foreign_cache_and_download_then_allow_empty_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let cache_path = directory.path().join("models.json");
    let bytes = display_document(Domain::Skills, 2, json!([]));
    let foreign = TranslationCatalog::parse(manifest(2, &bytes), &bytes).unwrap();
    write_cache(&cache_path, &foreign).await.unwrap();
    let initial = TranslationCatalog::bundled_display(Domain::Models);
    let (sender, receiver) = watch::channel(Arc::clone(&initial));
    let server = MockServer::start().await;
    run_worker_with(
        sender,
        Some(cache_path),
        false,
        watch::channel(true).1,
        Arc::clone(&initial),
        server.uri(),
    )
    .await;
    assert_eq!(receiver.borrow().domain(), Some(Domain::Models));
    assert_eq!(receiver.borrow().version(), 1);
    assert!(server.received_requests().await.unwrap().is_empty());
    serve_manifest(&server, &foreign.manifest).await;
    Mock::given(path("/catalogs/2.json"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&server)
        .await;
    assert!(
        refresh_catalog(&client(), &server.uri(), &initial)
            .await
            .is_err()
    );
    server.reset().await;
    let bytes = display_document(Domain::Models, 2, json!([]));
    serve_manifest(&server, &manifest(2, &bytes)).await;
    Mock::given(path("/catalogs/2.json"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&server)
        .await;
    let empty = refresh_catalog(&client(), &server.uri(), &initial)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(empty.domain(), Some(Domain::Models));
    assert!(
        empty
            .display_catalog()
            .unwrap()
            .lookup(
                "description",
                &["grok-4.6"],
                "SpaceXAI's latest frontier model"
            )
            .is_none()
    );
}

#[tokio::test]
async fn display_cache_keeps_newest_snapshot_offline_and_rejects_a_late_writer() {
    let directory = tempfile::tempdir().unwrap();
    let cache = directory.path().join("settings.json");
    let bytes = display_document(
        Domain::Settings,
        3,
        json!([{
            "field":"tip", "context":[], "source":"Future tip", "translation":"未来提示"
        }]),
    );
    let newer = TranslationCatalog::parse(manifest(3, &bytes), &bytes).unwrap();
    let bytes = display_document(Domain::Settings, 2, json!([]));
    let older = TranslationCatalog::parse(manifest(2, &bytes), &bytes).unwrap();
    write_cache(&cache, &newer).await.unwrap();
    write_cache(&cache, &older).await.unwrap();
    assert_eq!(read_cache(&cache).await.unwrap().version(), 3);
    let initial = TranslationCatalog::bundled_display(Domain::Settings);
    let (sender, mut receiver) = watch::channel(Arc::clone(&initial));
    let server = MockServer::start().await;
    run_worker_with(
        sender,
        Some(cache),
        false,
        watch::channel(true).1,
        initial,
        server.uri(),
    )
    .await;
    receiver.changed().await.unwrap();
    assert_eq!(receiver.borrow_and_update().version(), 3);
    assert_eq!(
        receiver
            .borrow()
            .display_catalog()
            .unwrap()
            .lookup("tip", &[], "Future tip"),
        Some("未来提示")
    );
    assert!(receiver.changed().await.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
}

fn document(version: u64, text: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "schema_version": 1,
        "version": version,
        "locale": "zh-CN",
        "entries": [{"field": "title", "source": "From the team", "translation": text}]
    }))
    .unwrap()
}

fn manifest(version: u64, bytes: &[u8]) -> Manifest {
    Manifest {
        schema_version: 1,
        version,
        sha256: digest(bytes),
    }
}

fn catalog(version: u64, text: &str) -> TranslationCatalog {
    let bytes = document(version, text);
    TranslationCatalog::parse(manifest(version, &bytes), &bytes).unwrap()
}

fn client() -> reqwest::Client {
    xai_grok_extra_ca::build_reqwest_client(|builder| {
        builder
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
    })
    .unwrap()
}

async fn serve_manifest(server: &MockServer, manifest: &Manifest) {
    Mock::given(method("GET"))
        .and(path("/manifest.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(manifest))
        .mount(server)
        .await;
}

#[test]
fn bundled_catalog_has_known_notices_and_keeps_exact_match_boundaries() {
    let catalog = TranslationCatalog::bundled();
    assert_eq!(catalog.version(), 1);
    assert_eq!(
        catalog.lookup(TranslationField::Title, "From the team"),
        Some("团队寄语")
    );
    assert_eq!(
        catalog.lookup(
            TranslationField::Message,
            "Hope you are having a wonderful day!"
        ),
        Some("祝你今天过得愉快！")
    );
    assert_eq!(
        catalog.lookup(TranslationField::Title, "From the team!"),
        None
    );
    assert_eq!(
        catalog.lookup(TranslationField::Title, " From the team"),
        None
    );
    assert_eq!(
        catalog.lookup(TranslationField::Message, "From the team"),
        None
    );
    assert_eq!(
        catalog.lookup(TranslationField::Title, "Future official notice"),
        None
    );
}

#[tokio::test]
async fn production_client_rejects_plain_http_before_sending_any_request() {
    let server = MockServer::start().await;
    let client = build_client().unwrap();
    assert!(
        read_response(&client, &server.uri(), MAX_MANIFEST_BYTES)
            .await
            .is_err()
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn offline_worker_loads_verified_cache_and_rejects_a_corrupt_cache() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("catalog.json");
    write_cache(&path, &catalog(2, "离线新版")).await.unwrap();
    let (sender, mut receiver) = watch::channel(TranslationCatalog::bundled());
    run_worker(sender, Some(path.clone()), false, watch::channel(false).1).await;
    receiver.changed().await.unwrap();
    assert_eq!(receiver.borrow_and_update().version(), 2);
    assert_eq!(
        receiver
            .borrow()
            .lookup(TranslationField::Title, "From the team"),
        Some("离线新版")
    );
    assert!(
        receiver.changed().await.is_err(),
        "offline worker must exit after loading cache"
    );

    tokio::fs::write(&path, b"broken cache").await.unwrap();
    let (sender, receiver) = watch::channel(TranslationCatalog::bundled());
    run_worker(sender, Some(path), false, watch::channel(false).1).await;
    assert_eq!(receiver.borrow().version(), 1);
}

#[tokio::test]
async fn dropping_translation_owner_cancels_an_inflight_request() {
    let server = MockServer::start().await;
    Mock::given(path("/manifest.json"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(30)))
        .mount(&server)
        .await;
    let (sender, receiver) = watch::channel(TranslationCatalog::bundled());
    let (trigger, load_started) = watch::channel(false);
    // The official load may start before the worker is first scheduled.
    trigger.send_replace(true);
    let task = tokio::spawn(refresh_loop(
        sender,
        TranslationCatalog::bundled(),
        None,
        RefreshSource {
            client: Some(client()),
            base_url: server.uri(),
            timeout: REFRESH_TIMEOUT,
        },
        load_started,
    ));
    let abort = task.abort_handle();
    let owner = TranslationUpdates { receiver, task };
    tokio::time::timeout(Duration::from_secs(1), async {
        while server.received_requests().await.unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(owner.current().version(), 1);
    drop(owner);
    tokio::time::timeout(Duration::from_secs(1), async {
        while !abort.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[test]
fn catalog_rejects_untrusted_shape_digest_and_duplicate_mappings() {
    let bytes = document(2, "新版团队寄语");
    assert!(TranslationCatalog::parse(manifest(3, &bytes), &bytes).is_err());
    let mut bad_hash = manifest(2, &bytes);
    bad_hash.sha256 = "0".repeat(64);
    assert!(TranslationCatalog::parse(bad_hash, &bytes).is_err());
    for field in ["id", "severity", "url", "expires_at"] {
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["entries"][0]["field"] = json!(field);
        let encoded = serde_json::to_vec(&value).unwrap();
        assert!(TranslationCatalog::parse(manifest(2, &encoded), &encoded).is_err());
    }
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let duplicate = value["entries"][0].clone();
    value["entries"].as_array_mut().unwrap().push(duplicate);
    let encoded = serde_json::to_vec(&value).unwrap();
    assert!(TranslationCatalog::parse(manifest(2, &encoded), &encoded).is_err());
    value["entries"].as_array_mut().unwrap().pop();
    value["entries"][0]["translation"] = json!("\u{001b}[2J");
    let encoded = serde_json::to_vec(&value).unwrap();
    assert!(TranslationCatalog::parse(manifest(2, &encoded), &encoded).is_err());
    value["entries"][0]["translation"] = json!("翻译");
    value["schema_version"] = json!(2);
    let encoded = serde_json::to_vec(&value).unwrap();
    assert!(TranslationCatalog::parse(manifest(2, &encoded), &encoded).is_err());
}

#[test]
fn catalog_rejects_oversize_and_allows_an_explicit_empty_new_version() {
    let bytes = vec![b' '; MAX_CATALOG_BYTES + 1];
    assert!(TranslationCatalog::parse(manifest(2, &bytes), &bytes).is_err());
    let bytes = serde_json::to_vec(
        &json!({"schema_version": 1, "version": 2, "locale": "zh-CN", "entries": []}),
    )
    .unwrap();
    let empty = TranslationCatalog::parse(manifest(2, &bytes), &bytes).unwrap();
    assert_eq!(empty.lookup(TranslationField::Title, "From the team"), None);
}

#[tokio::test]
async fn same_or_older_version_never_downloads_a_catalog() {
    let server = MockServer::start().await;
    let current = TranslationCatalog::bundled();
    serve_manifest(&server, &current.manifest).await;
    assert!(
        refresh_catalog(&client(), &server.uri(), &current)
            .await
            .unwrap()
            .is_none()
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), "/manifest.json");

    server.reset().await;
    serve_manifest(&server, &manifest(1, BUNDLED_CATALOG.as_bytes())).await;
    assert!(
        refresh_catalog(&client(), &server.uri(), &catalog(2, "新版"))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn changed_version_downloads_validates_and_replaces_the_entire_catalog() {
    let server = MockServer::start().await;
    let bytes = document(2, "新版团队寄语");
    serve_manifest(&server, &manifest(2, &bytes)).await;
    Mock::given(path("/catalogs/2.json"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .expect(1)
        .mount(&server)
        .await;
    let newer = refresh_catalog(&client(), &server.uri(), &TranslationCatalog::bundled())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(newer.version(), 2);
    assert_eq!(
        newer.lookup(TranslationField::Title, "From the team"),
        Some("新版团队寄语")
    );
    // An entry removed by the maintainer stays removed; a newer catalog is not
    // merged with stale bundled or cached translations.
    assert_eq!(
        newer.lookup(TranslationField::Title, "Degraded performance"),
        None
    );
}

#[tokio::test]
async fn forbidden_redirect_and_oversize_manifest_do_not_fetch_catalogs() {
    for response in [
        ResponseTemplate::new(403),
        ResponseTemplate::new(429),
        ResponseTemplate::new(302).insert_header("Location", "https://example.invalid/other.json"),
        ResponseTemplate::new(200).set_body_bytes(vec![b' '; MAX_MANIFEST_BYTES + 1]),
    ] {
        let server = MockServer::start().await;
        Mock::given(path("/manifest.json"))
            .respond_with(response)
            .mount(&server)
            .await;
        assert!(
            refresh_catalog(&client(), &server.uri(), &TranslationCatalog::bundled())
                .await
                .is_err()
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn reused_version_or_bad_download_is_rejected_without_mutating_cache() {
    let dir = tempfile::tempdir().unwrap();
    let cache_path = dir.path().join("catalog.json");
    let current = TranslationCatalog::bundled();
    write_cache(&cache_path, &current).await.unwrap();
    let original = tokio::fs::read(&cache_path).await.unwrap();
    let server = MockServer::start().await;
    serve_manifest(&server, &manifest(1, &document(1, "非法覆盖"))).await;
    assert!(
        refresh_catalog(&client(), &server.uri(), &current)
            .await
            .is_err()
    );
    assert_eq!(server.received_requests().await.unwrap().len(), 1);

    server.reset().await;
    serve_manifest(&server, &manifest(2, &document(2, "新版"))).await;
    Mock::given(path("/catalogs/2.json"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(document(2, "错误的下载内容")))
        .mount(&server)
        .await;
    assert!(
        refresh_catalog(&client(), &server.uri(), &current)
            .await
            .is_err()
    );
    assert_eq!(tokio::fs::read(&cache_path).await.unwrap(), original);
    assert_eq!(read_cache(&cache_path).await.unwrap().version(), 1);
}

#[tokio::test]
async fn cache_is_atomic_digest_checked_and_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let cache_path = dir.path().join("nested/catalog.json");
    write_cache(&cache_path, &catalog(2, "第二版"))
        .await
        .unwrap();
    write_cache(&cache_path, &catalog(3, "第三版"))
        .await
        .unwrap();
    let cached = read_cache(&cache_path).await.unwrap();
    assert_eq!(cached.version(), 3);
    assert_eq!(
        cached.lookup(TranslationField::Title, "From the team"),
        Some("第三版")
    );
    let files = std::fs::read_dir(cache_path.parent().unwrap())
        .unwrap()
        .count();
    assert_eq!(files, 2);
    assert!(cache_lock_path(&cache_path).is_file());
    let original = tokio::fs::read(&cache_path).await.unwrap();
    let mut corrupt: CachedCatalog = serde_json::from_slice(&original).unwrap();
    corrupt.catalog_json = corrupt.catalog_json.replace("第三版", "篡改版");
    tokio::fs::write(&cache_path, serde_json::to_vec(&corrupt).unwrap())
        .await
        .unwrap();
    assert!(read_cache(&cache_path).await.is_err());
    tokio::fs::write(&cache_path, vec![b' '; MAX_CACHE_BYTES + 1])
        .await
        .unwrap();
    assert!(read_cache(&cache_path).await.is_err());
}

#[tokio::test]
async fn failed_cache_replacement_cleans_its_temporary_file_and_preserves_existing_data() {
    let dir = tempfile::tempdir().unwrap();
    let cache_path = dir.path().join("catalog.json");
    // A directory at the target forces rename to fail after the temporary
    // file has been fully written, exercising the writer's cleanup path.
    tokio::fs::create_dir(&cache_path).await.unwrap();
    let existing = cache_path.join("existing.txt");
    tokio::fs::write(&existing, b"keep me").await.unwrap();
    assert!(
        write_cache(&cache_path, &catalog(2, "第二版"))
            .await
            .is_err()
    );
    assert_eq!(tokio::fs::read(existing).await.unwrap(), b"keep me");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    assert!(cache_lock_path(&cache_path).is_file());
}

#[tokio::test]
async fn timed_out_writer_cannot_replace_a_newer_cached_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let cache_path = dir.path().join("catalog.json");
    let old = catalog(2, "迟到的旧版");
    let bytes = old.cache_bytes().unwrap();
    let old_path = cache_path.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (resume_tx, resume_rx) = std::sync::mpsc::channel();
    let mut old_writer = tokio::task::spawn_blocking(move || {
        started_tx.send(()).unwrap();
        resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        write_cache_bytes(&old_path, &bytes, &old.manifest)
    });
    started_rx.await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut old_writer)
            .await
            .is_err()
    );
    write_cache(&cache_path, &catalog(3, "最新版本"))
        .await
        .unwrap();
    resume_tx.send(()).unwrap();
    old_writer.await.unwrap().unwrap();
    let cached = read_cache(&cache_path).await.unwrap();
    assert_eq!(cached.version(), 3);
    assert_eq!(
        cached.lookup(TranslationField::Title, "From the team"),
        Some("最新版本")
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[tokio::test]
async fn cache_commit_lock_serializes_writers_and_rejects_reused_versions() {
    let dir = tempfile::tempdir().unwrap();
    let cache_path = dir.path().join("catalog.json");
    let lock = lock_cache_commit(&cache_path).unwrap();
    let contender = std::fs::File::options()
        .read(true)
        .write(true)
        .open(cache_lock_path(&cache_path))
        .unwrap();
    assert!(matches!(
        contender.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    drop(lock);
    contender.try_lock().unwrap();
    drop(contender);
    write_cache(&cache_path, &catalog(3, "第三版"))
        .await
        .unwrap();
    let original = tokio::fs::read(&cache_path).await.unwrap();
    assert!(
        write_cache(&cache_path, &catalog(3, "非法重用"))
            .await
            .is_err()
    );
    assert_eq!(tokio::fs::read(&cache_path).await.unwrap(), original);
}

#[tokio::test]
async fn stalled_refresh_leaves_snapshot_available_and_does_not_retry_immediately() {
    let server = MockServer::start().await;
    Mock::given(path("/manifest.json"))
        .respond_with(ResponseTemplate::new(403).set_delay(Duration::from_millis(300)))
        .mount(&server)
        .await;
    let current = TranslationCatalog::bundled();
    let (sender, receiver) = watch::channel(Arc::clone(&current));
    let (trigger, load_started) = watch::channel(false);
    trigger.send_replace(true);
    let worker = tokio::spawn(refresh_loop(
        sender,
        current,
        None,
        RefreshSource {
            client: Some(client()),
            base_url: server.uri(),
            timeout: Duration::from_millis(50),
        },
        load_started,
    ));
    // A UI/input task remains schedulable while the network request is stalled.
    tokio::time::timeout(Duration::from_millis(100), tokio::task::yield_now())
        .await
        .unwrap();
    assert_eq!(
        receiver
            .borrow()
            .lookup(TranslationField::Title, "From the team"),
        Some("团队寄语")
    );
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    assert!(!receiver.has_changed().unwrap());
    drop(receiver);
    tokio::time::timeout(Duration::from_millis(100), worker)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn background_refresh_notifies_without_waiting_for_cache_write() {
    let server = MockServer::start().await;
    let bytes = document(2, "实时中文");
    serve_manifest(&server, &manifest(2, &bytes)).await;
    Mock::given(path("/catalogs/2.json"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let invalid_parent = dir.path().join("not-a-directory");
    tokio::fs::write(&invalid_parent, "existing data")
        .await
        .unwrap();
    let (sender, mut receiver) = watch::channel(TranslationCatalog::bundled());
    let (trigger, load_started) = watch::channel(false);
    trigger.send_replace(true);
    let worker = tokio::spawn(refresh_loop(
        sender,
        TranslationCatalog::bundled(),
        Some(invalid_parent.join("catalog.json")),
        RefreshSource {
            client: Some(client()),
            base_url: server.uri(),
            timeout: Duration::from_secs(1),
        },
        load_started,
    ));
    tokio::time::timeout(Duration::from_secs(1), receiver.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        receiver
            .borrow_and_update()
            .lookup(TranslationField::Title, "From the team"),
        Some("实时中文")
    );
    drop(receiver);
    tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        tokio::fs::read_to_string(invalid_parent).await.unwrap(),
        "existing data"
    );
}

#[tokio::test]
async fn elapsed_time_never_starts_a_check_without_an_official_load() {
    let server = MockServer::start().await;
    serve_manifest(&server, &TranslationCatalog::bundled().manifest).await;
    let (sender, receiver) = watch::channel(TranslationCatalog::bundled());
    let (trigger, load_started) = watch::channel(false);
    let worker = tokio::spawn(refresh_loop(
        sender,
        TranslationCatalog::bundled(),
        None,
        RefreshSource {
            client: Some(client()),
            base_url: server.uri(),
            timeout: REFRESH_TIMEOUT,
        },
        load_started,
    ));
    tokio::task::yield_now().await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(30 * 60)).await;
    tokio::task::yield_now().await;
    assert!(server.received_requests().await.unwrap().is_empty());
    tokio::time::resume();
    trigger.send_replace(true);
    wait_for_request_count(&server, 1).await;
    drop(receiver);
    tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn concurrent_loads_are_coalesced_and_a_later_load_checks_again() {
    use tokio::io::AsyncWriteExt as _;

    // Explicitly hold responses until the test releases them. This guarantees
    // that duplicate signals arrive in flight, independent of host scheduling.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (request_sender, mut requests) = tokio::sync::mpsc::unbounded_channel();
    let response_gate = Arc::new(tokio::sync::Semaphore::new(0));
    let server_gate = Arc::clone(&response_gate);
    let bytes = document(2, "事件驱动新版");
    let manifest_bytes = serde_json::to_vec(&manifest(2, &bytes)).unwrap();
    let server = tokio::spawn(async move {
        for index in 1..=3 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(socket.read_u8().await.unwrap());
                assert!(request.len() < 8192);
            }
            request_sender.send(index).unwrap();
            server_gate.acquire().await.unwrap().forget();
            let body = if request.starts_with(b"GET /catalogs/2.json ") {
                &bytes
            } else {
                assert!(request.starts_with(b"GET /manifest.json "));
                &manifest_bytes
            };
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(headers.as_bytes()).await.unwrap();
            socket.write_all(body).await.unwrap();
        }
    });
    let (sender, mut receiver) = watch::channel(TranslationCatalog::bundled());
    let (trigger, load_started) = watch::channel(false);
    // Multiple loads before the worker runs also coalesce into one check.
    for _ in 0..5 {
        trigger.send_replace(true);
    }
    let source = RefreshSource {
        client: Some(client()),
        base_url,
        timeout: REFRESH_TIMEOUT,
    };
    let worker = tokio::spawn(refresh_loop(
        sender,
        TranslationCatalog::bundled(),
        None,
        source,
        load_started,
    ));
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), requests.recv())
            .await
            .unwrap(),
        Some(1)
    );
    for _ in 0..5 {
        trigger.send_replace(true);
    }
    response_gate.add_permits(2);
    tokio::time::timeout(Duration::from_secs(1), receiver.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receiver.borrow_and_update().version(), 2);
    assert_eq!(requests.recv().await, Some(2));
    assert!(
        tokio::time::timeout(Duration::from_millis(100), requests.recv())
            .await
            .is_err(),
        "in-flight loads must not queue an extra check"
    );
    trigger.send_replace(true);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), requests.recv())
            .await
            .unwrap(),
        Some(3)
    );
    drop(receiver);
    tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap();
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn worker_stops_while_waiting_when_load_source_closes() {
    let (sender, receiver) = watch::channel(TranslationCatalog::bundled());
    let (trigger, load_started) = watch::channel(false);
    let worker = tokio::spawn(refresh_loop(
        sender,
        TranslationCatalog::bundled(),
        None,
        RefreshSource {
            client: None,
            base_url: "https://example.invalid".into(),
            timeout: REFRESH_TIMEOUT,
        },
        load_started,
    ));
    drop(trigger);
    tokio::time::timeout(Duration::from_secs(1), worker)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receiver.borrow().version(), 1);
}

async fn wait_for_request_count(server: &MockServer, count: usize) {
    // Also bounded while a test has paused Tokio's clock.
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while server.received_requests().await.unwrap().len() < count {
        assert!(
            std::time::Instant::now() < deadline,
            "request did not arrive"
        );
        tokio::task::yield_now().await;
    }
}
