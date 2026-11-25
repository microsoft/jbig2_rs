#[cfg(verus_keep_ghost)]
use super::halftone_spec;
use super::{HalftoneError, MalformedError};
use crate::DecodeError;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[derive(Debug)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) enum GrayIndexes {
    Narrow(Vec<u8>),
    Wide(Vec<u32>),
}

impl GrayIndexes {
    #[cfg(test)]
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == halftone_spec::index_values(self).len(),
    ))]
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Narrow(values) => values.len(),
            Self::Wide(values) => values.len(),
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == if index < halftone_spec::index_values(self).len() {
            Some(halftone_spec::index_values(self)[index as int])
        } else { None },
    ))]
    pub(super) fn get(&self, index: usize) -> Option<u32> {
        let value = match self {
            Self::Narrow(values) => u32::from(*values.get(index)?),
            Self::Wide(values) => *values.get(index)?,
        };
        Some(value)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(values) => halftone_spec::index_values(&values).len() == length
                && forall|i: int| 0 <= i < length ==>
                    #[trigger] halftone_spec::index_values(&values)[i] == 0,
            Err(_) => true,
        },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(super) fn zeroed(length: usize, depth: u8) -> Result<Self, HalftoneError> {
        let bytes = u64::try_from(length)
            .map_err(|_| MalformedError::IntegerOverflow)?
            .checked_mul(if depth <= 8 { 1 } else { 4 })
            .ok_or(MalformedError::IntegerOverflow)?;
        if depth <= 8 {
            let mut values = Vec::new();
            values
                .try_reserve(length)
                .map_err(|_| DecodeError::Allocation {
                    requested_bytes: bytes,
                })?;
            values.resize(length, 0_u8);
            Ok(Self::Narrow(values))
        } else {
            let mut values = Vec::new();
            values
                .try_reserve(length)
                .map_err(|_| DecodeError::Allocation {
                    requested_bytes: bytes,
                })?;
            values.resize(length, 0_u32);
            Ok(Self::Wide(values))
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures halftone_spec::index_values(final(self)).len()
            == halftone_spec::index_values(old(self)).len(),
    ))]
    pub(super) fn accumulate_byte(
        &mut self,
        offset: usize,
        byte: u8,
        plane: u8,
    ) -> Result<(), HalftoneError> {
        match self {
            Self::Narrow(values) => {
                if plane >= 8 {
                    return Err(HalftoneError::InvalidGrayBitDepth(plane));
                }
                let end = offset
                    .checked_add(8)
                    .ok_or(MalformedError::IntegerOverflow)?;
                let chunk = values
                    .get_mut(offset..end)
                    .ok_or(MalformedError::IntegerOverflow)?;
                chunk[0] |= ((byte >> 7) & 1) << plane;
                chunk[1] |= ((byte >> 6) & 1) << plane;
                chunk[2] |= ((byte >> 5) & 1) << plane;
                chunk[3] |= ((byte >> 4) & 1) << plane;
                chunk[4] |= ((byte >> 3) & 1) << plane;
                chunk[5] |= ((byte >> 2) & 1) << plane;
                chunk[6] |= ((byte >> 1) & 1) << plane;
                chunk[7] |= (byte & 1) << plane;
                Ok(())
            }
            Self::Wide(values) => {
                if plane >= 32 {
                    return Err(HalftoneError::InvalidGrayBitDepth(plane));
                }
                super::planes::accumulate_gray_byte(values, offset, byte, plane)
            }
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures halftone_spec::index_values(final(self)).len()
            == halftone_spec::index_values(old(self)).len(),
    ))]
    pub(super) fn insert_bit(&mut self, offset: usize, plane: u8) -> Result<(), HalftoneError> {
        match self {
            Self::Narrow(values) => {
                if plane >= 8 {
                    return Err(HalftoneError::InvalidGrayBitDepth(plane));
                }
                let value = values
                    .get_mut(offset)
                    .ok_or(MalformedError::IntegerOverflow)?;
                *value |= 1_u8 << plane;
            }
            Self::Wide(values) => {
                if plane >= 32 {
                    return Err(HalftoneError::InvalidGrayBitDepth(plane));
                }
                let value = values
                    .get_mut(offset)
                    .ok_or(MalformedError::IntegerOverflow)?;
                *value |= 1_u32 << plane;
            }
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures match out {
            Ok(()) => halftone_spec::indexes_valid(halftone_spec::index_values(self), count),
            Err(_) => !halftone_spec::indexes_valid(halftone_spec::index_values(self), count),
        },
    ))]
    pub(super) fn validate(&self, count: u64) -> Result<(), HalftoneError> {
        match self {
            Self::Wide(values) => super::render::validate_pattern_indices(values, count),
            Self::Narrow(values) => {
                let mut position = 0_usize;
                #[cfg_attr(verus_keep_ghost, verus_spec(
                    invariant
                        *self == Self::Narrow(*values),
                        position <= values@.len(),
                        forall|i: int| 0 <= i < position ==>
                            (values@[i] as u64) < count,
                    decreases values@.len() - position,
                ))]
                while position < values.len() {
                    let index = u32::from(values[position]);
                    if u64::from(index) >= count {
                        #[cfg(verus_keep_ghost)]
                        proof! {
                            let projected = halftone_spec::index_values(self);
                            assert(projected.len() == values@.len());
                            assert(projected[position as int] == index);
                            if halftone_spec::indexes_valid(projected, count) {
                                assert((projected[position as int] as u64) < count);
                                assert(false);
                            }
                        }
                        return Err(HalftoneError::InvalidPatternIndex {
                            index,
                            pattern_count: count,
                        });
                    }
                    position += 1;
                }
                Ok(())
            }
        }
    }

    #[cfg(test)]
    pub(super) fn test_values(&self) -> Vec<u32> {
        (0..self.len())
            .map(|index| self.get(index).unwrap())
            .collect()
    }
}
