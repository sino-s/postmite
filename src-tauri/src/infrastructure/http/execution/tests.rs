#[cfg(test)]
mod tests {
    use std::{
        io::Write,
        sync::{Arc, Mutex},
        time::{Duration, SystemTime},
    };

    use rcgen::{
        BasicConstraints, CertificateParams, CertifiedKey, DistinguishedName, DnType,
        ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose, PKCS_RSA_SHA256,
    };
    use tokio::{
        io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::{mpsc, oneshot, Mutex as AsyncMutex},
    };
    use tokio_rustls::{
        rustls::{
            pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
            server::WebPkiClientVerifier,
            version::TLS12,
            RootCertStore, ServerConfig,
        },
        TlsAcceptor,
    };

    use super::*;
    use crate::{
        application::execution::{ExecutionCoordinator, ExecutionEventKind, ExecutionRequest},
        domain::request::{
            BodyFilePath, BodyFileReference, MultipartPart, OrderedField, ProxyPolicy, ProxySource,
            RequestBody, RequestContent, RequestDraftId, TimeoutPolicy, TlsPolicy, TransportPolicy,
        },
    };

    static TLS_TEST_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

    #[tokio::test]
    async fn get_reaches_local_fixture_with_ordered_query_and_headers() {
        let (server, captured) = start_fixture(FixtureMode::Echo).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/echo?ignored=true"),
            query: vec![
                field(1, "tag", "second"),
                field(0, "tag", "first"),
                OrderedField {
                    enabled: false,
                    order: 2,
                    name: "skip".to_owned(),
                    value: "no".to_owned(),
                },
            ],
            headers: vec![
                field(1, "x-duplicate", "two"),
                field(0, "x-duplicate", "one"),
            ],
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("fixture request");
        assert!(request.starts_with("GET /echo?tag=first&tag=second HTTP/1.1"));
        assert!(request.contains("\r\nx-duplicate: one\r\n"));
        assert!(request.contains("\r\nx-duplicate: two\r\n"));
        assert_terminal_completed(&events);
        assert_ordered_sequences(&events);
    }

    #[tokio::test]
    async fn auth_headers_and_query_are_sent_after_resolution() {
        let (server, captured) = start_fixture(FixtureMode::Echo).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/auth"),
            query: vec![field(0, "api_key", "query-token")],
            headers: vec![field(0, "Authorization", "Bearer header-token")],
            tls: crate::domain::request::TlsPolicy {
                verify: false,
                ..Default::default()
            },
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("fixture request");
        assert!(request.starts_with("GET /auth?api_key=query-token HTTP/1.1"));
        assert!(request.contains("\r\nauthorization: Bearer header-token\r\n"));
        assert!(events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Started {
                tls_verification: false,
                ..
            }
        )));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn redirects_follow_up_to_policy_limit_and_emit_chain() {
        let (server, captured) = start_redirect_fixture().await;
        let events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/login"),
            body: RequestBody::Raw {
                content: "payload".to_owned(),
            },
            ..RequestContent::blank()
        })
        .await;

        let requests = captured.await.expect("fixture requests");
        assert!(requests[0].starts_with("POST /login HTTP/1.1"));
        assert!(requests[0].ends_with("payload"));
        assert!(requests[1].starts_with("GET /session HTTP/1.1"));
        assert!(!requests[1].ends_with("payload"));
        assert!(events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Redirected {
                status: 303,
                to,
                ..
            } if to.ends_with("/session")
        )));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn certificate_reference_errors_are_safe() {
        let (server, _captured) = start_fixture(FixtureMode::Echo).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/cert"),
            tls: crate::domain::request::TlsPolicy {
                custom_ca_reference: Some("../ca.pem".to_owned()),
                ..Default::default()
            },
            ..RequestContent::blank()
        })
        .await;

        assert!(events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Failed { message } if message == "certificate.reference.invalid"
        )));
    }

    #[tokio::test]
    async fn invalid_certificate_fails_by_default() {
        let _tls_test_lock = TLS_TEST_LOCK.lock().await;
        let fixture = TlsFixture::new(false).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: fixture.url("/tls"),
            ..RequestContent::blank()
        })
        .await;

        assert!(events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Failed { message } if message == "transport.failed"
        )));
    }

    #[tokio::test]
    async fn custom_ca_fixture_succeeds_when_configured() {
        let _tls_test_lock = TLS_TEST_LOCK.lock().await;
        let fixture = TlsFixture::new(false).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: fixture.url("/tls"),
            tls: TlsPolicy {
                custom_ca_reference: Some(fixture.ca_path()),
                ..Default::default()
            },
            ..RequestContent::blank()
        })
        .await;

        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn mtls_fixture_succeeds_with_client_certificate_reference() {
        let _tls_test_lock = TLS_TEST_LOCK.lock().await;
        let fixture = TlsFixture::new(true).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: fixture.url("/mtls"),
            tls: TlsPolicy {
                custom_ca_reference: Some(fixture.ca_path()),
                client_certificate_reference: Some(fixture.client_cert_path()),
                client_key_reference: Some(fixture.client_key_path()),
                ..Default::default()
            },
            ..RequestContent::blank()
        })
        .await;

        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn custom_authenticated_proxy_is_used_without_exposing_credentials() {
        let (proxy, captured) = start_authenticated_proxy_fixture().await;
        let expected_proxy = format!("http://{}", proxy.address);
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: "http://example.test/proxied".to_owned(),
            transport: TransportPolicy {
                proxy: ProxyPolicy {
                    source: ProxySource::Custom,
                    url: Some(format!("http://user:fixture-pass@{}", proxy.address)),
                    no_proxy: Vec::new(),
                },
                ..TransportPolicy::default()
            },
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("proxy request");
        assert!(request.starts_with("GET http://example.test/proxied HTTP/1.1"));
        assert!(
            request.contains("\r\nproxy-authorization: Basic dXNlcjpmaXh0dXJlLXBhc3M=\r\n"),
            "unexpected proxy request:\n{request}"
        );
        assert!(events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Started { proxy, .. }
                if proxy.source == "custom"
                    && proxy.selected_proxy.as_deref() == Some(expected_proxy.as_str())
                    && proxy.bypass_reason.is_none()
        )));
        assert!(!format!("{events:?}").contains("fixture-pass"));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn custom_no_proxy_bypasses_proxy_for_matching_host() {
        let (server, captured) = start_fixture(FixtureMode::Echo).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/direct"),
            transport: TransportPolicy {
                proxy: ProxyPolicy {
                    source: ProxySource::Custom,
                    url: Some("http://127.0.0.1:9".to_owned()),
                    no_proxy: vec!["127.0.0.1".to_owned()],
                },
                ..TransportPolicy::default()
            },
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("direct request");
        assert!(request.starts_with("GET /direct HTTP/1.1"));
        assert!(events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Started { proxy, .. }
                if proxy.source == "custom"
                    && proxy.selected_proxy.is_none()
                    && proxy.bypass_reason.as_deref() == Some("no_proxy.custom")
        )));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn deterministic_overall_timeout_is_classified() {
        let (server, _captured) = start_fixture(FixtureMode::SlowHeaders).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/overall-timeout"),
            transport: TransportPolicy {
                timeouts: TimeoutPolicy {
                    connect_ms: 0,
                    overall_ms: 50,
                    idle_ms: 0,
                },
                ..TransportPolicy::default()
            },
            ..RequestContent::blank()
        })
        .await;

        assert!(events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Failed { message } if message == "timeout.overall"
        )));
    }

    #[tokio::test]
    async fn deterministic_idle_timeout_is_classified() {
        let (server, _captured) = start_fixture(FixtureMode::SlowDownload).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/idle-timeout"),
            transport: TransportPolicy {
                timeouts: TimeoutPolicy {
                    connect_ms: 0,
                    overall_ms: 0,
                    idle_ms: 250,
                },
                ..TransportPolicy::default()
            },
            ..RequestContent::blank()
        })
        .await;

        assert!(events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Failed { message } if message == "timeout.idle"
        )));
    }

    #[tokio::test]
    async fn protocol_metadata_is_reported_for_http11_fixture() {
        let (server, _captured) = start_fixture(FixtureMode::Echo).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/protocol"),
            ..RequestContent::blank()
        })
        .await;

        assert!(events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::ResponseHeaders { protocol, remote_addr, .. }
                if protocol == "HTTP/1.1" && remote_addr.is_some()
        )));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn compressed_responses_decode_and_report_wire_and_decoded_sizes() {
        for encoding in ["gzip", "br", "deflate", "zstd"] {
            let decoded = format!("decoded body for {encoding}");
            let encoded = encode_fixture_body(encoding, decoded.as_bytes());
            let encoded_len = encoded.len() as u64;
            let decoded_len = decoded.len() as u64;
            let (server, _captured) = start_fixture(FixtureMode::Compressed {
                encoding,
                body: encoded,
            })
            .await;
            let events = run_fixture_request(RequestContent {
                method: "GET".to_owned(),
                url: server.url("/compressed"),
                ..RequestContent::blank()
            })
            .await;

            assert!(
                events.iter().any(|event| matches!(
                    &event.kind,
                    ExecutionEventKind::Completed {
                        body_preview,
                        decoded_bytes,
                        wire_bytes,
                        ..
                    } if body_preview == &decoded
                        && *decoded_bytes == decoded_len
                        && *wire_bytes == Some(encoded_len)
                )),
                "missing decoded completion for {encoding}: {events:?}"
            );
        }
    }

    #[tokio::test]
    async fn response_above_preview_limit_spools_to_temporary_file() {
        let body = vec![b'a'; MAX_RESPONSE_PREVIEW_BYTES + 1];
        let expected_len = body.len() as u64;
        let (server, _captured) = start_fixture(FixtureMode::Body { body }).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/large"),
            ..RequestContent::blank()
        })
        .await;

        let response_file = events.iter().find_map(|event| match &event.kind {
            ExecutionEventKind::Completed {
                body_preview,
                body_truncated,
                decoded_bytes,
                response_file,
                ..
            } => {
                assert_eq!(body_preview.len(), MAX_RESPONSE_PREVIEW_BYTES);
                assert!(*body_truncated);
                assert_eq!(*decoded_bytes, expected_len);
                response_file.clone()
            }
            _ => None,
        });
        let response_file = response_file.expect("spooled response file metadata");
        assert_eq!(response_file.byte_count, expected_len);
        assert_eq!(
            std::fs::metadata(&response_file.path)
                .expect("spooled response file")
                .len(),
            expected_len
        );
        std::fs::remove_file(response_file.path).expect("remove spooled response");
    }

    #[tokio::test]
    async fn response_at_preview_limit_does_not_create_temporary_file() {
        let body = vec![b'b'; MAX_RESPONSE_PREVIEW_BYTES];
        let expected_len = body.len() as u64;
        let (server, _captured) = start_fixture(FixtureMode::Body { body }).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/preview-limit"),
            ..RequestContent::blank()
        })
        .await;

        assert!(events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Completed {
                body_preview,
                body_truncated: false,
                decoded_bytes,
                response_file: None,
                ..
            } if body_preview.len() == MAX_RESPONSE_PREVIEW_BYTES
                && *decoded_bytes == expected_len
        )));
    }

    #[tokio::test]
    async fn compressed_decoded_response_spools_without_unbounded_ipc_body() {
        let decoded = vec![b'z'; MAX_RESPONSE_PREVIEW_BYTES + 1];
        let expected_len = decoded.len() as u64;
        let encoded = encode_fixture_body("gzip", &decoded);
        let (server, _captured) = start_fixture(FixtureMode::Compressed {
            encoding: "gzip",
            body: encoded,
        })
        .await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/compressed-large"),
            ..RequestContent::blank()
        })
        .await;

        let response_file = events.iter().find_map(|event| match &event.kind {
            ExecutionEventKind::Completed {
                body_preview,
                body_truncated,
                decoded_bytes,
                response_file,
                ..
            } => {
                assert_eq!(body_preview.len(), MAX_RESPONSE_PREVIEW_BYTES);
                assert!(*body_truncated);
                assert_eq!(*decoded_bytes, expected_len);
                response_file.clone()
            }
            _ => None,
        });
        let response_file = response_file.expect("spooled compressed response file metadata");
        assert_eq!(response_file.byte_count, expected_len);
        std::fs::remove_file(response_file.path).expect("remove spooled compressed response");
    }

    #[test]
    fn collector_removes_incomplete_temporary_output_after_limit_failure() {
        let mut collector = ResponseBodyCollector::new(
            ExecutionId::new(),
            ResponseCollectionLimits {
                preview_bytes: 4,
                normal_decoded_bytes: 8,
            },
            SystemTime::now(),
        )
        .expect("collector");
        collector.push(b"abcdef").expect("spool response");
        let path = collector
            .spool
            .as_ref()
            .expect("spool file")
            .path()
            .to_path_buf();

        let error = collector.push(b"ghi").expect_err("limit failure");
        assert!(matches!(error, HttpExecutionError::ResponseTooLarge));
        drop(collector);

        assert!(!path.exists(), "incomplete spool file should be deleted");
    }

    #[test]
    fn collector_rejects_decoded_body_at_normal_execution_boundary() {
        let mut collector = ResponseBodyCollector::new(
            ExecutionId::new(),
            ResponseCollectionLimits {
                preview_bytes: 4,
                normal_decoded_bytes: 8,
            },
            SystemTime::now(),
        )
        .expect("collector");

        collector.push(b"abcdefgh").expect("boundary body");
        let error = collector.push(b"i").expect_err("decoded boundary");

        assert!(matches!(error, HttpExecutionError::ResponseTooLarge));
    }

    #[tokio::test]
    #[ignore = "streams more than 1 GiB to verify the normal execution boundary"]
    async fn near_one_gib_response_stops_at_normal_execution_boundary() {
        cleanup_all_response_temp_files();
        let (server, _captured) = start_fixture(FixtureMode::NearOneGibBoundary).await;
        let events = run_fixture_request(RequestContent {
            method: "GET".to_owned(),
            url: server.url("/near-one-gib"),
            ..RequestContent::blank()
        })
        .await;

        assert!(events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Failed { message } if message == "response.too_large"
        )));
        cleanup_all_response_temp_files();
    }

    #[test]
    fn cleanup_removes_response_temp_files_after_retention_window() {
        let directory = tempfile::TempDir::new().expect("response temp directory");
        let file_path = directory.path().join("response-expired.tmp");
        std::fs::write(&file_path, b"expired").expect("write expired file");

        cleanup_response_temp_files(
            directory.path().to_path_buf(),
            SystemTime::now() + Duration::from_secs(RESPONSE_TEMP_RETENTION_SECONDS + 1),
            response_temp_retention(),
        )
        .expect("cleanup response temp files");

        assert!(
            !file_path.exists(),
            "expired response temp file should be removed"
        );
    }

    #[tokio::test]
    async fn eight_concurrent_responses_complete_with_timing_metadata() {
        let server = start_multi_response_fixture(8).await;
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));

        for index in 0..8 {
            coordinator
                .start(
                    ExecutionRequest {
                        draft_id: RequestDraftId::new(),
                        workspace_base_directory: None,
                        content: RequestContent {
                            url: server.url(&format!("/concurrent/{index}")),
                            ..RequestContent::blank()
                        },
                    },
                    event_sink(&events),
                    run_http_execution,
                )
                .expect("start concurrent execution");
        }

        wait_until(&events, |events| {
            events
                .iter()
                .filter(|event| matches!(event.kind, ExecutionEventKind::Completed { .. }))
                .count()
                == 8
        })
        .await;
        let events = events.lock().expect("lock events").clone();
        let completions = events
            .iter()
            .filter_map(|event| match &event.kind {
                ExecutionEventKind::Completed { timing, .. } => Some(timing),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(completions.len(), 8);
        assert!(completions
            .iter()
            .all(|timing| timing.first_byte_ms.is_some() && timing.download_ms.is_some()));
    }

    #[tokio::test]
    async fn post_reaches_local_fixture_with_raw_body_without_cors_headers() {
        let (server, captured) = start_fixture(FixtureMode::Echo).await;
        let events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/post"),
            body: RequestBody::Raw {
                content: "{\"ok\":true}".to_owned(),
            },
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("fixture request");
        assert!(
            request.starts_with("POST /post HTTP/1.1"),
            "unexpected request:\n{request}"
        );
        assert!(request.ends_with("{\"ok\":true}"));
        assert_terminal_completed(&events);
        assert!(events.iter().any(|event| {
            matches!(
                event.kind,
                ExecutionEventKind::UploadProgress {
                    sent_bytes: 11,
                    total_bytes: 11
                }
            )
        }));
    }

    #[tokio::test]
    async fn url_encoded_body_reaches_local_fixture_byte_for_byte() {
        let (server, captured) = start_fixture(FixtureMode::Echo).await;
        let events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/form"),
            body: RequestBody::UrlEncoded {
                fields: vec![field(1, "space", "a b"), field(0, "tag", "first")],
            },
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("fixture request");
        assert!(request.contains("content-type: application/x-www-form-urlencoded"));
        assert!(request.ends_with("tag=first&space=a+b"));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn binary_body_streams_file_to_local_fixture_byte_for_byte() {
        let file = tempfile::NamedTempFile::new().expect("temporary body file");
        std::fs::write(file.path(), b"binary-payload").expect("write body file");
        let metadata = std::fs::metadata(file.path()).expect("body metadata");
        let (server, captured) = start_fixture(FixtureMode::Echo).await;

        let events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/binary"),
            body: RequestBody::Binary {
                file: BodyFileReference {
                    path: BodyFilePath::Absolute {
                        path: file.path().to_string_lossy().into_owned(),
                    },
                    file_name: "payload.bin".to_owned(),
                    size: metadata.len(),
                    modified_at_epoch_seconds: None,
                    sha256: hash_bytes(b"binary-payload"),
                },
            },
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("fixture request");
        assert!(request.starts_with("POST /binary HTTP/1.1"));
        assert!(request.ends_with("binary-payload"));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn relative_binary_body_survives_base_directory_move() {
        let base = tempfile::TempDir::new().expect("base directory");
        let file_path = base.path().join("payload.bin");
        std::fs::write(&file_path, b"relative-payload").expect("write body file");
        let metadata = std::fs::metadata(&file_path).expect("body metadata");
        let (server, captured) = start_fixture(FixtureMode::Echo).await;

        let events = run_fixture_request_with_base(
            RequestContent {
                method: "POST".to_owned(),
                url: server.url("/relative"),
                body: RequestBody::Binary {
                    file: BodyFileReference {
                        path: BodyFilePath::Relative {
                            path: "payload.bin".to_owned(),
                        },
                        file_name: "payload.bin".to_owned(),
                        size: metadata.len(),
                        modified_at_epoch_seconds: None,
                        sha256: hash_bytes(b"relative-payload"),
                    },
                },
                ..RequestContent::blank()
            },
            Some(base.path().to_string_lossy().into_owned()),
        )
        .await;

        let request = captured.await.expect("fixture request");
        assert!(request.ends_with("relative-payload"));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn changed_or_missing_body_files_fail_before_upload() {
        let changed = tempfile::NamedTempFile::new().expect("changed body file");
        std::fs::write(changed.path(), b"changed").expect("write body file");
        let (server, _captured) = start_fixture(FixtureMode::Echo).await;
        let changed_events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/changed"),
            body: RequestBody::Binary {
                file: BodyFileReference {
                    path: BodyFilePath::Absolute {
                        path: changed.path().to_string_lossy().into_owned(),
                    },
                    file_name: "payload.bin".to_owned(),
                    size: 999,
                    modified_at_epoch_seconds: None,
                    sha256: hash_bytes(b"original"),
                },
            },
            ..RequestContent::blank()
        })
        .await;
        assert!(changed_events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Failed { message } if message == "body.file.changed"
        )));

        let missing_path = changed.path().with_file_name("missing.bin");
        let (server, _captured) = start_fixture(FixtureMode::Echo).await;
        let missing_events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/missing"),
            body: RequestBody::Binary {
                file: BodyFileReference {
                    path: BodyFilePath::Absolute {
                        path: missing_path.to_string_lossy().into_owned(),
                    },
                    file_name: "missing.bin".to_owned(),
                    size: 7,
                    modified_at_epoch_seconds: None,
                    sha256: hash_bytes(b"missing"),
                },
            },
            ..RequestContent::blank()
        })
        .await;
        assert!(missing_events.iter().any(|event| matches!(
            &event.kind,
            ExecutionEventKind::Failed { message } if message == "body.file.missing"
        )));
    }

    #[tokio::test]
    async fn multipart_body_streams_fields_and_files_to_local_fixture() {
        let file = tempfile::NamedTempFile::new().expect("temporary body file");
        std::fs::write(file.path(), b"file-payload").expect("write body file");
        let metadata = std::fs::metadata(file.path()).expect("body metadata");
        let (server, captured) = start_fixture(FixtureMode::Echo).await;

        let events = run_fixture_request(RequestContent {
            method: "POST".to_owned(),
            url: server.url("/multipart"),
            body: RequestBody::Multipart {
                parts: vec![
                    MultipartPart::Field {
                        enabled: true,
                        order: 0,
                        name: "kind".to_owned(),
                        value: "fixture".to_owned(),
                    },
                    MultipartPart::File {
                        enabled: true,
                        order: 1,
                        name: "upload".to_owned(),
                        file: BodyFileReference {
                            path: BodyFilePath::Absolute {
                                path: file.path().to_string_lossy().into_owned(),
                            },
                            file_name: "payload.txt".to_owned(),
                            size: metadata.len(),
                            modified_at_epoch_seconds: None,
                            sha256: hash_bytes(b"file-payload"),
                        },
                    },
                ],
            },
            ..RequestContent::blank()
        })
        .await;

        let request = captured.await.expect("fixture request");
        let lower_request = request.to_ascii_lowercase();
        assert!(lower_request.contains("content-disposition: form-data; name=\"kind\""));
        assert!(request.contains("fixture"));
        assert!(lower_request
            .contains("content-disposition: form-data; name=\"upload\"; filename=\"payload.txt\""));
        assert!(request.contains("file-payload"));
        assert_terminal_completed(&events);
    }

    #[tokio::test]
    async fn cancel_before_connect_emits_one_terminal_event() {
        let (server, _captured) = start_fixture(FixtureMode::SlowHeaders).await;
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            workspace_base_directory: None,
            content: RequestContent {
                url: server.url("/slow"),
                ..RequestContent::blank()
            },
        };
        let sink = event_sink(&events);
        let started = coordinator
            .start(request, sink, run_http_execution)
            .expect("start execution");

        coordinator
            .cancel(started.execution_id)
            .expect("cancel execution");
        let events = wait_for_terminal(events).await;

        assert_one_cancelled_terminal(&events);
    }

    #[tokio::test]
    async fn cancel_during_download_emits_one_terminal_event() {
        let (server, _captured) = start_fixture(FixtureMode::SlowDownload).await;
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            workspace_base_directory: None,
            content: RequestContent {
                url: server.url("/download"),
                ..RequestContent::blank()
            },
        };
        let sink = event_sink(&events);
        let started = coordinator
            .start(request, sink, run_http_execution)
            .expect("start execution");

        wait_until(&events, |events| {
            events
                .iter()
                .any(|event| matches!(event.kind, ExecutionEventKind::DownloadProgress { .. }))
        })
        .await;
        coordinator
            .cancel(started.execution_id)
            .expect("cancel execution");
        let events = wait_for_terminal(events).await;

        assert_one_cancelled_terminal(&events);
    }

    #[tokio::test]
    async fn cancel_during_spooled_download_removes_incomplete_temporary_output() {
        cleanup_all_response_temp_files();
        let (server, _captured) = start_fixture(FixtureMode::SlowLargeDownload).await;
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            workspace_base_directory: None,
            content: RequestContent {
                url: server.url("/large-download"),
                ..RequestContent::blank()
            },
        };
        let sink = event_sink(&events);
        let started = coordinator
            .start(request, sink, run_http_execution)
            .expect("start execution");

        wait_until(&events, |events| {
            events.iter().any(|event| {
                matches!(
                    event.kind,
                    ExecutionEventKind::DownloadProgress { received_bytes, .. }
                        if received_bytes > MAX_RESPONSE_PREVIEW_BYTES as u64
                )
            })
        })
        .await;
        coordinator
            .cancel(started.execution_id)
            .expect("cancel execution");
        let events = wait_for_terminal(events).await;

        assert_one_cancelled_terminal(&events);
        assert!(!response_temp_files_for_execution(started.execution_id)
            .expect("list response temp files")
            .iter()
            .any(|path| path.exists()));
    }

    #[tokio::test]
    async fn cancel_during_upload_emits_one_terminal_event() {
        let (server, _captured) = start_fixture(FixtureMode::SlowUploadRead).await;
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            workspace_base_directory: None,
            content: RequestContent {
                method: "POST".to_owned(),
                url: server.url("/upload"),
                body: RequestBody::Raw {
                    content: "x".repeat(128 * 1024),
                },
                ..RequestContent::blank()
            },
        };
        let sink = event_sink(&events);
        let started = coordinator
            .start(request, sink, run_http_execution)
            .expect("start execution");

        wait_until(&events, |events| {
            events
                .iter()
                .any(|event| matches!(event.kind, ExecutionEventKind::UploadProgress { .. }))
        })
        .await;
        coordinator
            .cancel(started.execution_id)
            .expect("cancel execution");
        let events = wait_for_terminal(events).await;

        assert_one_cancelled_terminal(&events);
    }

    async fn run_fixture_request(content: RequestContent) -> Vec<ExecutionEvent> {
        run_fixture_request_with_base(content, None).await
    }

    async fn run_fixture_request_with_base(
        content: RequestContent,
        workspace_base_directory: Option<String>,
    ) -> Vec<ExecutionEvent> {
        let coordinator = Arc::new(ExecutionCoordinator::new());
        let events = Arc::new(Mutex::new(Vec::new()));
        let request = ExecutionRequest {
            draft_id: RequestDraftId::new(),
            workspace_base_directory,
            content,
        };
        let sink = event_sink(&events);

        coordinator
            .start(request, sink, run_http_execution)
            .expect("start execution");

        wait_for_terminal(events).await
    }

    fn event_sink(
        events: &Arc<Mutex<Vec<ExecutionEvent>>>,
    ) -> Arc<dyn Fn(ExecutionEvent) + Send + Sync + 'static> {
        let events = Arc::clone(events);
        Arc::new(move |event| {
            events.lock().expect("lock events").push(event);
        })
    }

    async fn wait_for_terminal(events: Arc<Mutex<Vec<ExecutionEvent>>>) -> Vec<ExecutionEvent> {
        wait_until(&events, |events| {
            events.iter().any(|event| event.kind.is_terminal())
        })
        .await;
        events.lock().expect("lock events").clone()
    }

    async fn wait_until(
        events: &Arc<Mutex<Vec<ExecutionEvent>>>,
        predicate: impl Fn(&[ExecutionEvent]) -> bool,
    ) {
        for _ in 0..3_000 {
            {
                let events = events.lock().expect("lock events");
                if predicate(&events) {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for execution event");
    }

    fn assert_terminal_completed(events: &[ExecutionEvent]) {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind.is_terminal())
                .count(),
            1,
            "expected one terminal event, got: {events:#?}"
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event.kind, ExecutionEventKind::Completed { .. })),
            "expected completed event, got: {events:#?}"
        );
    }

    fn assert_one_cancelled_terminal(events: &[ExecutionEvent]) {
        assert_eq!(
            events
                .iter()
                .filter(|event| event.kind.is_terminal())
                .count(),
            1
        );
        assert!(events
            .iter()
            .any(|event| matches!(event.kind, ExecutionEventKind::Cancelled)));
    }

    fn assert_ordered_sequences(events: &[ExecutionEvent]) {
        for (index, event) in events.iter().enumerate() {
            assert_eq!(event.sequence, index as u64 + 1);
        }
    }

    fn field(order: u32, name: &str, value: &str) -> OrderedField {
        OrderedField {
            enabled: true,
            order,
            name: name.to_owned(),
            value: value.to_owned(),
        }
    }

    fn hash_bytes(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    struct TlsFixture {
        address: String,
        _directory: tempfile::TempDir,
        ca_path: PathBuf,
        client_cert_path: PathBuf,
        client_key_path: PathBuf,
    }

    impl TlsFixture {
        async fn new(require_client_cert: bool) -> Self {
            let directory = tempfile::TempDir::new().expect("tls fixture directory");
            let certificates = TestCertificates::new();
            let ca_path = directory.path().join("ca.pem");
            let client_cert_path = directory.path().join("client.pem");
            let client_key_path = directory.path().join("client.key");
            std::fs::write(&ca_path, certificates.ca_pem()).expect("write ca");
            std::fs::write(&client_cert_path, certificates.client_cert_pem())
                .expect("write client cert");
            std::fs::write(&client_key_path, certificates.client_key_pem())
                .expect("write client key");

            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind tls fixture");
            let port = listener.local_addr().expect("tls address").port();
            let address = format!("localhost:{port}");
            let server_config = certificates.server_config(require_client_cert);
            let acceptor = TlsAcceptor::from(Arc::new(server_config));
            let (ready_tx, mut ready_rx) = mpsc::channel(1);

            tokio::spawn(async move {
                ready_tx
                    .send(())
                    .await
                    .expect("signal tls fixture readiness");
                let (stream, _) = listener.accept().await.expect("accept tls request");
                let Ok(mut stream) = acceptor.accept(stream).await else {
                    return;
                };
                let _ = tokio::time::timeout(
                    Duration::from_millis(250),
                    read_http_request(&mut stream),
                )
                .await;
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                    .await;
            });

            ready_rx.recv().await.expect("tls fixture ready");
            Self {
                address,
                _directory: directory,
                ca_path,
                client_cert_path,
                client_key_path,
            }
        }

        fn url(&self, path: &str) -> String {
            format!("https://{}{}", self.address, path)
        }

        fn ca_path(&self) -> String {
            self.ca_path.to_string_lossy().into_owned()
        }

        fn client_cert_path(&self) -> String {
            self.client_cert_path.to_string_lossy().into_owned()
        }

        fn client_key_path(&self) -> String {
            self.client_key_path.to_string_lossy().into_owned()
        }
    }

    struct TestCertificates {
        ca: CertifiedKey,
        server: CertifiedKey,
        client: CertifiedKey,
    }

    impl TestCertificates {
        fn new() -> Self {
            let ca_key_pair = KeyPair::generate_for(&PKCS_RSA_SHA256).expect("ca key");
            let mut ca_params =
                CertificateParams::new(vec!["my.ca".to_owned()]).expect("ca params");
            ca_params.distinguished_name = distinguished_name("my.ca");
            ca_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
            ca_params.extended_key_usages = vec![
                ExtendedKeyUsagePurpose::ServerAuth,
                ExtendedKeyUsagePurpose::ClientAuth,
            ];
            let ca_cert = ca_params.self_signed(&ca_key_pair).expect("ca cert");
            let ca = CertifiedKey {
                cert: ca_cert,
                key_pair: ca_key_pair,
            };

            let server = signed_certificate(
                &ca,
                vec!["localhost".to_owned()],
                "localhost",
                ExtendedKeyUsagePurpose::ServerAuth,
            );
            let client = signed_certificate(
                &ca,
                vec!["postmite-client".to_owned()],
                "postmite-client",
                ExtendedKeyUsagePurpose::ClientAuth,
            );

            Self { ca, server, client }
        }

        fn ca_pem(&self) -> String {
            self.ca.cert.pem()
        }

        fn client_cert_pem(&self) -> String {
            self.client.cert.pem()
        }

        fn client_key_pem(&self) -> String {
            self.client.key_pair.serialize_pem()
        }

        fn server_config(&self, require_client_cert: bool) -> ServerConfig {
            let certificate_chain = vec![CertificateDer::from(self.server.cert.der().to_vec())];
            let private_key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(
                self.server.key_pair.serialize_der(),
            ));
            let builder = ServerConfig::builder_with_protocol_versions(&[&TLS12]);
            let mut config = if require_client_cert {
                let mut roots = RootCertStore::empty();
                roots
                    .add(CertificateDer::from(self.ca.cert.der().to_vec()))
                    .expect("add client root");
                let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
                    .build()
                    .expect("client verifier");
                builder
                    .with_client_cert_verifier(verifier)
                    .with_single_cert(certificate_chain, private_key)
                    .expect("mtls server config")
            } else {
                builder
                    .with_no_client_auth()
                    .with_single_cert(certificate_chain, private_key)
                    .expect("tls server config")
            };
            config.alpn_protocols = vec![b"http/1.1".to_vec()];
            config
        }
    }

    fn signed_certificate(
        issuer: &CertifiedKey,
        subject_alt_names: Vec<String>,
        common_name: &str,
        usage: ExtendedKeyUsagePurpose,
    ) -> CertifiedKey {
        let key_pair = KeyPair::generate_for(&PKCS_RSA_SHA256).expect("certificate key");
        let mut params = CertificateParams::new(subject_alt_names).expect("certificate params");
        params.distinguished_name = distinguished_name(common_name);
        params.is_ca = IsCa::ExplicitNoCa;
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![usage];
        let cert = params
            .signed_by(&key_pair, &issuer.cert, &issuer.key_pair)
            .expect("signed certificate");
        CertifiedKey { cert, key_pair }
    }

    fn distinguished_name(common_name: &str) -> DistinguishedName {
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, common_name);
        distinguished_name
    }

    struct FixtureServer {
        address: String,
    }

    impl FixtureServer {
        fn url(&self, path: &str) -> String {
            format!("http://{}{}", self.address, path)
        }
    }

    enum FixtureMode {
        Echo,
        SlowHeaders,
        SlowDownload,
        SlowLargeDownload,
        SlowUploadRead,
        NearOneGibBoundary,
        Body {
            body: Vec<u8>,
        },
        Compressed {
            encoding: &'static str,
            body: Vec<u8>,
        },
    }

    async fn start_fixture(mode: FixtureMode) -> (FixtureServer, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address").to_string();
        let (captured_tx, captured_rx) = oneshot::channel();
        let (ready_tx, mut ready_rx) = mpsc::channel(1);

        tokio::spawn(async move {
            ready_tx.send(()).await.expect("signal fixture readiness");
            let (mut stream, _) = listener.accept().await.expect("accept fixture request");
            let request = read_http_request(&mut stream).await;
            let _ = captured_tx.send(request);

            match mode {
                FixtureMode::Echo => {
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: text/plain\r\n\r\nok",
                        )
                        .await
                        .expect("write echo response");
                }
                FixtureMode::SlowHeaders => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let _ = stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                        .await;
                }
                FixtureMode::SlowDownload => {
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\none")
                        .await
                        .expect("write first chunk");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let _ = stream.write_all(b"two").await;
                }
                FixtureMode::SlowLargeDownload => {
                    let total_bytes = MAX_RESPONSE_PREVIEW_BYTES + 2;
                    let response =
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {total_bytes}\r\n\r\n");
                    stream
                        .write_all(response.as_bytes())
                        .await
                        .expect("write large download headers");
                    stream
                        .write_all(&vec![b'x'; MAX_RESPONSE_PREVIEW_BYTES + 1])
                        .await
                        .expect("write first large chunk");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let _ = stream.write_all(b"y").await;
                }
                FixtureMode::NearOneGibBoundary => {
                    let total_bytes = MAX_NORMAL_RESPONSE_DECODED_BYTES + 1;
                    let response =
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {total_bytes}\r\n\r\n");
                    stream
                        .write_all(response.as_bytes())
                        .await
                        .expect("write near-boundary headers");
                    let chunk = vec![b'g'; 1024 * 1024];
                    let mut sent = 0_u64;
                    while sent < total_bytes {
                        let remaining = (total_bytes - sent) as usize;
                        let next_len = remaining.min(chunk.len());
                        if stream.write_all(&chunk[..next_len]).await.is_err() {
                            break;
                        }
                        sent += next_len as u64;
                    }
                }
                FixtureMode::SlowUploadRead => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let _ = stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                        .await;
                }
                FixtureMode::Body { body } => {
                    let response =
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
                    stream
                        .write_all(response.as_bytes())
                        .await
                        .expect("write body headers");
                    stream.write_all(&body).await.expect("write body");
                }
                FixtureMode::Compressed { encoding, body } => {
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Encoding: {encoding}\r\nContent-Length: {}\r\n\r\n",
                        body.len()
                    );
                    stream
                        .write_all(response.as_bytes())
                        .await
                        .expect("write compressed headers");
                    stream
                        .write_all(&body)
                        .await
                        .expect("write compressed body");
                }
            }
        });

        ready_rx.recv().await.expect("fixture ready");
        (FixtureServer { address }, captured_rx)
    }

    async fn start_multi_response_fixture(expected_requests: usize) -> FixtureServer {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind multi fixture");
        let address = listener
            .local_addr()
            .expect("multi fixture address")
            .to_string();
        let (ready_tx, mut ready_rx) = mpsc::channel(1);

        tokio::spawn(async move {
            ready_tx
                .send(())
                .await
                .expect("signal multi fixture readiness");
            for _ in 0..expected_requests {
                let (mut stream, _) = listener.accept().await.expect("accept multi request");
                tokio::spawn(async move {
                    let _ = read_http_request(&mut stream).await;
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: text/plain\r\n\r\nok",
                        )
                        .await
                        .expect("write multi response");
                });
            }
        });

        ready_rx.recv().await.expect("multi fixture ready");
        FixtureServer { address }
    }

    struct ProxyFixture {
        address: String,
    }

    async fn start_authenticated_proxy_fixture() -> (ProxyFixture, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind proxy fixture");
        let address = listener.local_addr().expect("proxy address").to_string();
        let (captured_tx, captured_rx) = oneshot::channel();
        let (ready_tx, mut ready_rx) = mpsc::channel(1);

        tokio::spawn(async move {
            ready_tx.send(()).await.expect("signal proxy readiness");
            let (mut stream, _) = listener.accept().await.expect("accept proxy request");
            let request = read_http_request(&mut stream).await;
            let _ = captured_tx.send(request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nproxied")
                .await
                .expect("write proxy response");
        });

        ready_rx.recv().await.expect("proxy ready");
        (ProxyFixture { address }, captured_rx)
    }

    fn encode_fixture_body(encoding: &str, body: &[u8]) -> Vec<u8> {
        match encoding {
            "gzip" => {
                let mut encoder =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(body).expect("gzip body");
                encoder.finish().expect("finish gzip")
            }
            "deflate" => {
                let mut encoder =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(body).expect("deflate body");
                encoder.finish().expect("finish deflate")
            }
            "br" => {
                let mut encoded = Vec::new();
                {
                    let mut encoder = brotli::CompressorWriter::new(&mut encoded, 4096, 5, 22);
                    encoder.write_all(body).expect("brotli body");
                }
                encoded
            }
            "zstd" => zstd::stream::encode_all(body, 0).expect("zstd body"),
            _ => body.to_vec(),
        }
    }

    async fn start_redirect_fixture() -> (FixtureServer, oneshot::Receiver<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind redirect fixture");
        let address = listener.local_addr().expect("fixture address").to_string();
        let (captured_tx, captured_rx) = oneshot::channel();
        let (ready_tx, mut ready_rx) = mpsc::channel(1);

        tokio::spawn(async move {
            ready_tx
                .send(())
                .await
                .expect("signal redirect fixture readiness");
            let mut requests = Vec::new();

            let (mut first_stream, _) = listener.accept().await.expect("accept first request");
            requests.push(read_http_request(&mut first_stream).await);
            first_stream
                .write_all(
                    b"HTTP/1.1 303 See Other\r\nLocation: /session\r\nContent-Length: 0\r\n\r\n",
                )
                .await
                .expect("write redirect");

            requests.push(read_http_request(&mut first_stream).await);
            first_stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .await
                .expect("write final response");

            let _ = captured_tx.send(requests);
        });

        ready_rx.recv().await.expect("fixture ready");
        (FixtureServer { address }, captured_rx)
    }

    async fn read_http_request(stream: &mut (impl AsyncRead + Unpin)) -> String {
        let mut buffer = Vec::new();
        let mut temp = [0_u8; 4096];
        loop {
            let read = stream.read(&mut temp).await.expect("read request");
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&temp[..read]);
            if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        let Some(header_end) = buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| index + 4)
        else {
            return String::from_utf8_lossy(&buffer).into_owned();
        };
        let headers = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.strip_prefix("content-length: ")
                    .or_else(|| line.strip_prefix("Content-Length: "))
            })
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);

        while buffer.len() < header_end + content_length {
            let read = stream.read(&mut temp).await.expect("read body");
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&temp[..read]);
        }

        String::from_utf8_lossy(&buffer).into_owned()
    }

    #[test]
    fn save_response_file_copies_only_current_temp_sources() {
        std::fs::create_dir_all(response_temp_dir()).expect("create response temp dir");
        let source = response_temp_dir().join(format!(
            "response-{}-save-test-decoded.tmp",
            ExecutionId::new()
        ));
        let destination_dir = tempfile::tempdir().expect("destination tempdir");
        let destination = destination_dir.path().join("response.bin");
        std::fs::write(&source, b"response-bytes").expect("write response temp source");

        let bytes = save_response_file(&source, &destination, SystemTime::now())
            .expect("save response file");

        assert_eq!(bytes, 14);
        assert_eq!(
            std::fs::read(&destination).expect("read copied response"),
            b"response-bytes"
        );
        std::fs::remove_file(source).expect("remove response temp source");
    }

    #[test]
    fn save_response_file_rejects_non_response_temp_sources() {
        let source_dir = tempfile::tempdir().expect("source tempdir");
        let source = source_dir.path().join("outside-response.tmp");
        let destination_dir = tempfile::tempdir().expect("destination tempdir");
        let destination = destination_dir.path().join("response.bin");
        std::fs::write(&source, b"secret").expect("write outside source");

        let error = save_response_file(&source, &destination, SystemTime::now())
            .expect_err("outside source rejected");

        assert!(matches!(error, HttpExecutionError::InvalidInput(_)));
        assert!(!destination.exists());
    }

    #[test]
    fn save_response_file_rejects_expired_response_temp_sources() {
        std::fs::create_dir_all(response_temp_dir()).expect("create response temp dir");
        let source = response_temp_dir().join(format!(
            "response-{}-expired-test-decoded.tmp",
            ExecutionId::new()
        ));
        let destination_dir = tempfile::tempdir().expect("destination tempdir");
        let destination = destination_dir.path().join("response.bin");
        std::fs::write(&source, b"expired").expect("write response temp source");
        let modified = std::fs::metadata(&source)
            .expect("source metadata")
            .modified()
            .expect("source modified");
        let after_expiry = modified + response_temp_retention() + Duration::from_secs(1);

        let error = save_response_file(&source, &destination, after_expiry)
            .expect_err("expired source rejected");

        assert!(matches!(error, HttpExecutionError::InvalidInput(_)));
        assert!(!destination.exists());
        std::fs::remove_file(source).expect("remove response temp source");
    }

    fn response_temp_files_for_execution(
        execution_id: ExecutionId,
    ) -> Result<Vec<PathBuf>, std::io::Error> {
        let Ok(entries) = std::fs::read_dir(response_temp_dir()) else {
            return Ok(Vec::new());
        };
        let execution_id = execution_id.to_string();
        let mut paths = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(&execution_id))
            {
                paths.push(path);
            }
        }
        Ok(paths)
    }
}
