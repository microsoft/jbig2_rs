// Source-only author navigation, not normative approval.
// Guide: ../README.md; audit ledger: ../../validation/traceability/requirements.json

use super::*;

verus! {

// Project framing model for the fields in T.88 (08/2018), 7.4.6.
pub(crate) open spec fn generic_header_length(flags: u8) -> nat {
    if flags & 1 != 0 { 18 }
    else if (flags >> 1) & 3 != 0 { 20 }
    else if flags & 16 != 0 { 42 }
    else { 26 }
}

// Model navigation: spec::bounded_arithmetic_tail [project-result-model]
// Purpose: Literal bounded FF AC tail checked by the existing generic/refinement helper.
// Audit: generic-bounded-arithmetic-tail
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE | ITU-T T.88 (08/2018) | E.3, Figure E.14, read CX / DECODE / Finished loop (p. 118)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn bounded_arithmetic_tail(data: Seq<u8>) -> bool {
    data.len() >= 2 && data[data.len() - 2] == 0xff && data[data.len() - 1] == 0xac
}

// Model navigation: spec::early_arithmetic_marker [project-result-model]
// Purpose: An existing strict marker-shaped pair before the accepted final tail.
// Audit: generic-early-arithmetic-marker
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE | ITU-T T.88 (08/2018) | E.3, Figure E.14, read CX / DECODE / Finished loop (p. 118)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn early_arithmetic_marker(data: Seq<u8>, offset: int) -> bool {
    0 <= offset && offset + 1 < data.len() && offset != data.len() - 2
        && data[offset] == 0xff && data[offset + 1] > 0x8f
}

// Model navigation: spec::arithmetic_terminator_validation [project-result-model]
// Purpose: Exact existing tail-error priority and earliest interior-marker result.
// Audit: generic-arithmetic-terminator-validation
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE | ITU-T T.88 (08/2018) | E.3, Figure E.14, read CX / DECODE / Finished loop (p. 118)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn arithmetic_terminator_validation(
    data: Seq<u8>,
    result: Result<(), crate::generic_region::GenericRegionError>,
) -> bool {
    match result {
        Ok(()) => bounded_arithmetic_tail(data)
            && forall|offset: int| 0 <= offset < data.len() - 1 ==>
                !early_arithmetic_marker(data, offset),
        Err(crate::generic_region::GenericRegionError::InvalidArithmeticTerminator) =>
            !bounded_arithmetic_tail(data),
        Err(crate::generic_region::GenericRegionError::UnexpectedArithmeticMarker {
            offset, code,
        }) => bounded_arithmetic_tail(data)
            && early_arithmetic_marker(data, offset as int)
            && code == data[offset as int + 1]
            && forall|before: int| 0 <= before < offset ==>
                !early_arithmetic_marker(data, before),
        _ => false,
    }
}

// Model navigation: spec::observed_marker_result [project-result-model]
// Purpose: Existing observed-marker acceptance and exact length/code/offset rejection.
// Audit: generic-observed-marker-result
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE | ITU-T T.88 (08/2018) | E.3, Figure E.14, read CX / DECODE / Finished loop (p. 118)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn observed_marker_result(
    marker: Option<crate::arithmetic::ArithmeticMarker>,
    coded_length: usize,
) -> Result<(), crate::generic_region::GenericRegionError> {
    match marker {
        None => Ok(()),
        Some(marker) => if coded_length < 2 {
            Err(crate::generic_region::GenericRegionError::InvalidArithmeticTerminator)
        } else if marker.code != 0xac || marker.offset != coded_length - 2 {
            Err(crate::generic_region::GenericRegionError::UnexpectedArithmeticMarker {
                offset: marker.offset, code: marker.code,
            })
        } else { Ok(()) },
    }
}

// Model navigation: spec::final_marker_result [project-result-model]
// Purpose: Exact session result composing observed-marker validation and final offset.
// Audit: generic-final-marker-result
// REQUIREMENT CONTEXT: T88-ARITHMETIC-DRIVER-CONTEXT-SEQUENCE | ITU-T T.88 (08/2018) | E.3, Figure E.14, read CX / DECODE / Finished loop (p. 118)
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn final_marker_result(
    marker: Option<crate::arithmetic::ArithmeticMarker>,
    coded_length: usize,
) -> Result<usize, crate::generic_region::GenericRegionError> {
    match observed_marker_result(marker, coded_length) {
        Err(error) => Err(error),
        Ok(()) => if coded_length >= 2 {
            Ok((coded_length - 2) as usize)
        } else {
            Err(crate::generic_region::GenericRegionError::InvalidArithmeticTerminator)
        },
    }
}

}
