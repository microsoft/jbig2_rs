// Project API results only; no normative statement or independent approval.
use super::*;

verus! {

// Model navigation: spec::global_parse_result [project-result-model]
// Purpose: Owned global input bytes, successful admission and first stream-limit error.
// Audit: document-global-parse-result
// REQUIREMENT CONTEXT: T88-PROFILE-REUSABLE-GLOBALS | ITU-T T.88 (08/2018) | D.3; 7.3.2 (embedded/resource context; reusable API is project policy)
// Fidelity: unresolved; no independent normative approval.
pub open spec fn global_parse_result(
    input: Seq<u8>,
    limits: crate::Limits,
    result: Result<crate::document::GlobalSegments, crate::DecodeError>,
) -> bool {
    &&& match result {
        Ok(globals) =>
            global_bytes(&globals) == input
            && input.len() <= limits.max_stream_bytes
            && input.len() <= limits.max_retained_state_bytes,
        Err(_) => true,
    }
    &&& if input.len() > u64::MAX {
        match result {
            Err(error) => error == crate::DecodeError::Malformed(
                crate::MalformedError::IntegerOverflow,
            ),
            Ok(_) => false,
        }
    } else if input.len() > limits.max_stream_bytes {
        match result {
            Err(error) => error == crate::DecodeError::LimitExceeded(crate::LimitExceeded {
                kind: crate::LimitKind::StreamBytes,
                limit: limits.max_stream_bytes,
                actual: input.len() as u64,
            }),
            Ok(_) => false,
        }
    } else {
        true
    }
}

}
