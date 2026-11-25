#![cfg(test)]

use super::*;
use std::path::PathBuf;

#[test]
#[ignore = "external hash-checked request streams; correctness only, no timing"]
fn prepared_globals_external_request_controls() {
    let list = PathBuf::from(
        std::env::var_os("JBIG2_PREPARED_REQUEST_LIST").expect("set request-list path"),
    );
    let expected_requests: usize = std::env::var("JBIG2_PREPARED_EXPECTED_REQUESTS")
        .unwrap_or_else(|_| "321".into())
        .parse()
        .unwrap();
    replay_request_list(&list, expected_requests, 2);
}

#[test]
#[ignore = "external single-use PDF control streams; no timing or owner reuse"]
fn prepared_globals_external_cold_controls() {
    let list = PathBuf::from(
        std::env::var_os("JBIG2_PREPARED_COLD_REQUEST_LIST").expect("set single-use request list"),
    );
    replay_request_list(&list, 1, 1);
}

fn replay_request_list(list: &std::path::Path, expected_requests: usize, repetitions: usize) {
    let root = list.parent().unwrap();
    let rows = std::fs::read_to_string(list).expect("read request list");
    let decoder = Decoder::default();
    let mut requests = 0;
    let mut metadata_refusals = 0;
    for row in rows
        .lines()
        .filter(|row| !row.is_empty() && !row.starts_with('#'))
    {
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 3, "id, image path, globals path or '-'");
        let input = std::fs::read(root.join(fields[1])).expect("read unchanged image stream");
        let bytes = if fields[2] == "-" {
            Vec::new()
        } else {
            std::fs::read(root.join(fields[2])).expect("read unchanged global stream")
        };
        requests += 1;
        let globals = match decoder.parse_globals(&bytes) {
            Ok(globals) => globals,
            Err(error) => {
                // This new API requires the unchanged parser's GlobalSegments.
                // A metadata refusal cannot reach owner creation.
                assert!(matches!(
                    &error,
                    DecodeError::Malformed(_)
                        | DecodeError::Unsupported(_)
                        | DecodeError::LimitExceeded(_)
                ));
                let repeated = decoder.parse_globals(&bytes).unwrap_err();
                assert_eq!(error_view(&error), error_view(&repeated));
                println!(
                    "prepared-public-metadata-refusal id={} error={error:?}",
                    fields[0]
                );
                metadata_refusals += 1;
                continue;
            }
        };
        for allowance in [0, 65_536] {
            // Per-request owners, never a cache keyed across document identities.
            let mut owner = decoder.prepared_globals(&globals, allowance);
            for _ in 0..repetitions {
                let expected = if bytes.is_empty() {
                    decoder.decode_embedded(&input)
                } else {
                    decoder.decode_embedded_with_globals(&input, &globals)
                };
                assert_outcome_eq!(
                    owner.decode_embedded(&input),
                    expected,
                    "request {}, allowance {}",
                    fields[0],
                    allowance,
                );
            }
            assert!(owner.info().resident_bytes <= owner.info().additional_storage_bytes);
            println!(
                "prepared-public-request id={} repetitions={repetitions} allowance={allowance} state={:?} resident_bytes={}",
                fields[0], owner.info().state, owner.info().resident_bytes
            );
        }
    }
    assert_eq!(requests, expected_requests);
    println!(
        "prepared-public-requests count={requests} metadata_refusals={metadata_refusals} exact=true"
    );
}
