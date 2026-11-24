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

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok((start, end)) =>
            start <= end
                && start as int == y as int * stride as int
                && end as int == (y as int + height as int) * stride as int
                && end as int - start as int == height as int * stride as int,
        Err(_) => true,
    },
))]
fn row_span(y: u32, height: u32, stride: usize) -> Result<(usize, usize), MalformedError> {
    let end_row = y
        .checked_add(height)
        .ok_or(MalformedError::IntegerOverflow)?;
    let start = usize::try_from(y)
        .map_err(|_| MalformedError::IntegerOverflow)?
        .checked_mul(stride)
        .ok_or(MalformedError::IntegerOverflow)?;
    let end = usize::try_from(end_row)
        .map_err(|_| MalformedError::IntegerOverflow)?
        .checked_mul(stride)
        .ok_or(MalformedError::IntegerOverflow)?;
    #[cfg(verus_keep_ghost)]
    proof! {
        vstd::arithmetic::mul::lemma_mul_is_distributive_add_other_way(
            stride as int, y as int, height as int,
        );
        vstd::arithmetic::mul::lemma_mul_inequality(
            y as int, y as int + height as int, stride as int,
        );
    }
    Ok((start, end))
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
    let source_column_end = source_column
        .checked_add(bytes)
        .ok_or(MalformedError::IntegerOverflow)?;
    let destination_column_end = destination_column
        .checked_add(bytes)
        .ok_or(MalformedError::IntegerOverflow)?;
    let source_stride = source.stride;
    let destination_stride = destination.stride;
    if source_column_end > source_stride || destination_column_end > destination_stride {
        return Err(MalformedError::IntegerOverflow);
    }
    let (source_start, source_end) = row_span(source_y, height, source_stride)?;
    let (destination_start, destination_end) = row_span(destination_y, height, destination_stride)?;
    let mut source_rows = source
        .data
        .get(source_start..source_end)
        .ok_or(MalformedError::IntegerOverflow)?;
    let Some(mut destination_rows) = destination.data.get_mut(destination_start..destination_end)
    else {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(destination);
            reveal(crate::spec::bitmap::storage_valid);
        }
        return Err(MalformedError::IntegerOverflow);
    };
    let mut row = 0_u32;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            row <= height,
            width > 0,
            height > 0,
            source_stride > 0,
            destination_stride > 0,
            source_rows@.len() == (height - row) as int * source_stride,
            destination_rows@.len() == (height - row) as int * destination_stride,
            source_column <= source_column_end <= source_stride,
            destination_column <= destination_column_end <= destination_stride,
            source_column_end - source_column == bytes,
            destination_column_end - destination_column == bytes,
        decreases height - row,
    ))]
    while row < height {
        #[cfg(verus_keep_ghost)]
        proof! {
            vstd::arithmetic::mul::lemma_mul_inequality(
                1, height as int - row as int, source_stride as int,
            );
            vstd::arithmetic::mul::lemma_mul_inequality(
                1, height as int - row as int, destination_stride as int,
            );
        }
        let (source_row, source_tail) = source_rows.split_at(source_stride);
        let (destination_row, destination_tail) = destination_rows.split_at_mut(destination_stride);
        source_rows = source_tail;
        destination_rows = destination_tail;
        let source_slice = &source_row[source_column..source_column_end];
        let destination_slice = &mut destination_row[destination_column..destination_column_end];
        if replace {
            copy_row_bytes(destination_slice, source_slice);
        } else {
            or_bytes(destination_slice, source_slice);
        }
        #[cfg(verus_keep_ghost)]
        proof! {
            vstd::arithmetic::mul::lemma_mul_is_distributive_sub_other_way(
                source_stride as int, height as int - row as int, 1,
            );
            vstd::arithmetic::mul::lemma_mul_is_distributive_sub_other_way(
                destination_stride as int, height as int - row as int, 1,
            );
            assert(source_rows@.len()
                == (height as int - row as int - 1) * source_stride as int);
            assert(destination_rows@.len()
                == (height as int - row as int - 1) * destination_stride as int);
        }
        row += 1;
    }
    #[cfg(verus_keep_ghost)]
    proof! {
        crate::views::bitmap_fields(destination);
        reveal(crate::spec::bitmap::storage_valid);
    }
    Ok(())
}
