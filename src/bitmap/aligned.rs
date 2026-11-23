use super::{access::copy_row_bytes, Bitmap};
use crate::MalformedError;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(
    requires old(destination)@.len() == source@.len(),
    ensures
        final(destination)@.len() == old(destination)@.len(),
        forall|index: int| 0 <= index < source@.len() ==>
            #[trigger] final(destination)@[index] == old(destination)@[index] | source@[index],
))]
fn or_bytes(destination: &mut [u8], source: &[u8]) {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= source@.len(),
            destination@.len() == source@.len(),
            forall|done: int| 0 <= done < index ==>
                #[trigger] destination@[done] == old(destination)@[done] | source@[done],
            forall|pending: int| index <= pending < source@.len() ==>
                #[trigger] destination@[pending] == old(destination)@[pending],
        decreases source@.len() - index,
    ))]
    while index < source.len() {
        destination[index] |= source[index];
        index += 1;
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires
        crate::spec::bitmap::storage_valid(old(destination)),
        crate::spec::bitmap::storage_valid(source),
        source_x as nat + width as nat <= source.width as nat,
        source_y as nat + height as nat <= source.height as nat,
        destination_x as nat + width as nat <= old(destination).width as nat,
        destination_y as nat + height as nat <= old(destination).height as nat,
        source_x % 8 == 0,
        destination_x % 8 == 0,
        width % 8 == 0,
    ensures
        crate::spec::bitmap::storage_valid(final(destination)),
        final(destination).width == old(destination).width,
        final(destination).height == old(destination).height,
        final(destination).stride == old(destination).stride,
        final(destination).data@.len() == old(destination).data@.len(),
        (width == 0 || height == 0) ==> final(destination).data@ == old(destination).data@,
        (width == 0 || height == 0) ==> out.is_ok(),
))]
pub(super) fn compose_region(
    destination: &mut Bitmap,
    source: &Bitmap,
    source_x: u32,
    source_y: u32,
    destination_x: u32,
    destination_y: u32,
    width: u32,
    height: u32,
    replace: bool,
) -> Result<(), MalformedError> {
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(destination);
        crate::views::bitmap_fields(source);
        reveal(crate::spec::bitmap::storage_valid);
    }
    if width == 0 || height == 0 {
        return Ok(());
    }
    let bytes = usize::try_from(width / 8).map_err(|_| MalformedError::IntegerOverflow)?;
    let source_column =
        usize::try_from(source_x / 8).map_err(|_| MalformedError::IntegerOverflow)?;
    let destination_column =
        usize::try_from(destination_x / 8).map_err(|_| MalformedError::IntegerOverflow)?;
    let mut row = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            row <= height,
            width > 0,
            height > 0,
            destination.width == old(destination).width,
            destination.height == old(destination).height,
            destination.stride == old(destination).stride,
            destination.data@.len() == old(destination).data@.len(),
            destination.stride as nat
                == crate::spec::bitmap::stride_for_width(destination.width as nat),
            destination.data@.len()
                == destination.stride as nat * destination.height as nat,
            crate::spec::bitmap::storage_valid(destination),
        decreases height - row,
    ))]
    while row < height {
        let source_row = usize::try_from(
            source_y
                .checked_add(row)
                .ok_or(MalformedError::IntegerOverflow)?,
        )
        .map_err(|_| MalformedError::IntegerOverflow)?;
        let source_start = source_row
            .checked_mul(source.stride)
            .and_then(|offset| offset.checked_add(source_column))
            .ok_or(MalformedError::IntegerOverflow)?;
        let source_end = source_start
            .checked_add(bytes)
            .ok_or(MalformedError::IntegerOverflow)?;
        let source_slice = source
            .data
            .get(source_start..source_end)
            .ok_or(MalformedError::IntegerOverflow)?;
        let destination_row = usize::try_from(
            destination_y
                .checked_add(row)
                .ok_or(MalformedError::IntegerOverflow)?,
        )
        .map_err(|_| MalformedError::IntegerOverflow)?;
        let destination_start = destination_row
            .checked_mul(destination.stride)
            .and_then(|offset| offset.checked_add(destination_column))
            .ok_or(MalformedError::IntegerOverflow)?;
        let destination_end = destination_start
            .checked_add(bytes)
            .ok_or(MalformedError::IntegerOverflow)?;
        let Some(destination_slice) = destination.data.get_mut(destination_start..destination_end)
        else {
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::views::bitmap_fields(destination);
                reveal(crate::spec::bitmap::storage_valid);
            }
            return Err(MalformedError::IntegerOverflow);
        };
        if replace {
            copy_row_bytes(destination_slice, source_slice);
        } else {
            or_bytes(destination_slice, source_slice);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(destination);
            reveal(crate::spec::bitmap::storage_valid);
        }
        row += 1;
    }
    Ok(())
}
