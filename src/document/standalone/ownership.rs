//! Witnesses for narrowly repairing an EOP association and closing its owner.

#[cfg(verus_keep_ghost)]
use super::policy_spec;
use super::{allocation_size, DecodeError, MalformedError, RecoveryKind};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(index) => index < pages@.len() && pages@[index as int] == page,
        None => forall|index: int| 0 <= index < pages@.len() ==> pages@[index] != page,
    },
))]
pub(super) fn open_index(pages: &[u32], page: u32) -> Option<usize> {
    let mut index = 0;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= pages@.len(),
            forall|prior: int| 0 <= prior < index ==> pages@[prior] != page,
        decreases pages@.len() - index,
    ))]
    while index < pages.len() {
        if pages[index] == page {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(result) => policy_spec::eop_ownership(
            associated, open@, closed@, result.0, result.1),
        Err(_) => !policy_spec::eop_recovery_allowed(associated, open@, closed@),
    },
))]
pub(super) fn recovery_owner(
    segment: u32,
    associated: u32,
    open: &[u32],
    closed: &[u32],
) -> Result<(u32, RecoveryKind), DecodeError> {
    let error = MalformedError::InvalidPageAssociation {
        segment,
        page: associated,
    };
    if open.len() != 1 || open[0] == 0 {
        return Err(error.into());
    }
    let owner = open[0];
    if associated == 0 {
        return Ok((owner, RecoveryKind::ZeroAssociatedEndOfPage));
    }
    if associated < owner {
        if let Some(_index) = open_index(closed, associated) {
            #[cfg(verus_keep_ghost)]
            proof! {
                assert(exists|witness: int| 0 <= witness < closed@.len()
                    && closed@[witness] == associated) by {
                    assert(closed@[_index as int] == associated);
                }
            }
            return Ok((owner, RecoveryKind::ClosedPageEndOfPage));
        }
    }
    Err(error.into())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires index < old(open)@.len(),
    ensures match out {
        Ok(page) =>
            page == old(open)@[index as int]
            && final(open)@ == old(open)@.remove(index as int)
            && final(closed)@ == old(closed)@.push(page),
        Err(_) => final(open)@ == old(open)@ && final(closed)@ == old(closed)@,
    },
))]
pub(super) fn close_page(
    open: &mut Vec<u32>,
    closed: &mut Vec<u32>,
    index: usize,
) -> Result<u32, DecodeError> {
    let count = closed
        .len()
        .checked_add(1)
        .ok_or(MalformedError::IntegerOverflow)?;
    let requested_bytes = allocation_size::<u32>(count)?;
    closed
        .try_reserve(1)
        .map_err(|_| DecodeError::Allocation { requested_bytes })?;
    let page = open.remove(index);
    closed.push(page);
    Ok(page)
}
