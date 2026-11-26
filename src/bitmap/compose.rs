use super::{Bitmap, CombinationOperator};
use crate::MalformedError;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

impl Bitmap {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            crate::spec::bitmap::storage_valid(old(self)),
            crate::spec::bitmap::storage_valid(source),
        ensures
            out.is_ok(),
            crate::spec::bitmap::storage_valid(final(self)),
            crate::views::bitmap_width(final(self))
                == crate::views::bitmap_width(old(self)),
            crate::views::bitmap_height(final(self))
                == crate::views::bitmap_height(old(self)),
            crate::views::bitmap_stride(final(self))
                == crate::views::bitmap_stride(old(self)),
            crate::views::bitmap_data(final(self)).len()
                == crate::views::bitmap_data(old(self)).len(),
            forall|index: int|
                0 <= index < crate::views::bitmap_data(old(self)).len()
                && !crate::spec::bitmap::compose_byte_may_change(
                    index,
                    crate::views::bitmap_stride(old(self)),
                    crate::views::bitmap_width(old(self)),
                    crate::views::bitmap_height(old(self)),
                    crate::views::bitmap_width(source),
                    crate::views::bitmap_height(source),
                    x,
                    y,
                )
                ==> crate::views::bitmap_data(final(self))[index]
                    == crate::views::bitmap_data(old(self))[index],
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn compose(
        &mut self,
        source: &Self,
        x: u32,
        y: u32,
        operator: CombinationOperator,
    ) -> Result<(), MalformedError> {
        let result = self.compose_chunked(source, x, y, operator);
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            crate::views::bitmap_fields(old(self));
            crate::views::bitmap_fields(source);
            reveal(crate::spec::bitmap::compose_frame);
            reveal(crate::spec::bitmap::storage_valid);
        }
        result
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            crate::spec::bitmap::storage_valid(old(self)),
            crate::spec::bitmap::storage_valid(source),
        ensures
            final(self).width == old(self).width,
            final(self).height == old(self).height,
            final(self).stride == old(self).stride,
            final(self).data@.len() == old(self).data@.len(),
            out.is_ok(),
            crate::spec::bitmap::compose_frame(
                old(self).data@, final(self).data@,
                old(self).stride, old(self).width, old(self).height,
                source.width, source.height, x, y,
            ),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::unnecessary_wraps)]
    fn compose_chunked(
        &mut self,
        source: &Self,
        x: u32,
        y: u32,
        operator: CombinationOperator,
    ) -> Result<(), MalformedError> {
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            crate::views::bitmap_fields(old(self));
            crate::views::bitmap_fields(source);
            reveal(crate::spec::bitmap::compose_frame);
        }
        let width = source.width.min(self.width.saturating_sub(x));
        let height = source.height.min(self.height.saturating_sub(y));
        #[cfg(verus_keep_ghost)]
        production_bitmap_clipped_bounds!(self, source, x, y);
        #[cfg(verus_keep_ghost)]
        production_bitmap_clipped_dimensions!(self, source, x, y, width, height);
        if width == 0 || height == 0 {
            return Ok(());
        }
        super::fast_compose::compose_rect_fast(self, source, 0, 0, x, y, width, height, operator);
        #[cfg(verus_keep_ghost)]
        proof! {
            crate::views::bitmap_fields(self);
            crate::views::bitmap_fields(old(self));
            crate::views::bitmap_fields(source);
            let stride = old(self).stride;
            let dst_col = (x as int) / 8;
            let dst_span = ((x as int) % 8 + width as int - 1) / 8 + 1;
            assert forall|index: int|
                0 <= index < old(self).data@.len()
                && !crate::spec::bitmap::compose_byte_may_change(
                    index, stride, old(self).width, old(self).height,
                    source.width, source.height, x, y,
                )
                implies self.data@[index] == old(self).data@[index]
            by {
                assert(stride > 0);
                let dy = index / (stride as int);
                let col = index % (stride as int);
                if (y as int) <= dy && dy < (y as int) + (height as int)
                    && dst_col <= col && col < dst_col + dst_span
                {
                    let row = dy - (y as int);
                    let sx = if col == dst_col {
                        0int
                    } else {
                        col * 8 - (x as int)
                    };
                    assert(0 <= sx < width as int);
                    assert(0 <= row < height as int);
                    assert(((x as int) + sx) / 8 == col);
                    vstd::arithmetic::div_mod::lemma_fundamental_div_mod(index, stride as int);
                    assert((stride as int) * dy == dy * (stride as int)) by (nonlinear_arith);
                    assert(crate::spec::bitmap::byte_index(
                        stride,
                        (x as int + sx) as u32,
                        (y as int + row) as u32,
                    ) as int == index);
                    assert(false);
                }
            };
        }
        Ok(())
    }
}
