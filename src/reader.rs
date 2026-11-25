use crate::MalformedError;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct ByteReader<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) position: usize,
}

impl<'a> ByteReader<'a> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out.bytes@ == bytes@, out.position == 0,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == self.position,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn position(&self) -> usize {
        self.position
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires self.position <= self.bytes@.len(),
        ensures out == self.bytes@.len() - self.position,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) const fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires self.position <= self.bytes@.len(),
        ensures out@ == self.bytes@.subrange(
            self.position as int,
            self.bytes@.len() as int,
        ),
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn remaining_bytes(&self) -> &'a [u8] {
        &self.bytes[self.position..self.bytes.len()]
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires old(self).position <= old(self).bytes@.len(),
        ensures
            final(self).bytes@ == old(self).bytes@,
            match out {
                Ok(value) =>
                    old(self).position < old(self).bytes@.len()
                    && final(self).position == old(self).position + 1
                    && value == old(self).bytes@[old(self).position as int],
                Err(_) =>
                    final(self).position == old(self).position
                    && old(self).bytes@.len() - old(self).position < 1,
            },
            match out {
                Err(error) => error == crate::spec::readers::eof_error(
                    old(self).position, 1, (old(self).bytes@.len() - old(self).position) as usize,
                ),
                Ok(_) => true,
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::question_mark)]
    pub(crate) fn read_u8(&mut self) -> Result<u8, MalformedError> {
        let bytes = match self.read_bytes(1) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        Ok(bytes[0])
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires old(self).position <= old(self).bytes@.len(),
        ensures
            final(self).bytes@ == old(self).bytes@,
            match out {
                Ok(value) =>
                    old(self).position + 4 <= old(self).bytes@.len()
                    && final(self).position == old(self).position + 4
                    && value
                        == ((old(self).bytes@[old(self).position as int] as u32) << 24)
                        | ((old(self).bytes@[(old(self).position + 1) as int] as u32) << 16)
                        | ((old(self).bytes@[(old(self).position + 2) as int] as u32) << 8)
                        | (old(self).bytes@[(old(self).position + 3) as int] as u32),
                Err(_) => final(self).position == old(self).position,
            },
            out.is_err() ==> old(self).bytes@.len() - old(self).position < 4,
            match out {
                Err(error) => error == crate::spec::readers::eof_error(
                    old(self).position, 4, (old(self).bytes@.len() - old(self).position) as usize,
                ),
                Ok(_) => true,
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn read_u32(&mut self) -> Result<u32, MalformedError> {
        let octets = match self.read_bytes(4) {
            Ok(value) => value,
            Err(error) => {
                #[cfg(verus_keep_ghost)]
                proof! {
                    assert(self.bytes@.len() - self.position < 4);
                }
                return Err(error);
            }
        };
        let byte0 = octets[0];
        let byte1 = octets[1];
        let byte2 = octets[2];
        let byte3 = octets[3];
        let value = (u32::from(byte0) << 24)
            | (u32::from(byte1) << 16)
            | (u32::from(byte2) << 8)
            | u32::from(byte3);
        Ok(value)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires old(self).position <= old(self).bytes@.len(),
        ensures
            final(self).bytes@ == old(self).bytes@,
            match out {
                Ok(value) =>
                    old(self).position + 2 <= old(self).bytes@.len()
                    && final(self).position == old(self).position + 2
                    && value
                        == ((old(self).bytes@[old(self).position as int] as u16) << 8)
                        | (old(self).bytes@[(old(self).position + 1) as int] as u16),
                Err(_) => final(self).position == old(self).position,
            },
            out.is_err() ==> old(self).bytes@.len() - old(self).position < 2,
            match out {
                Err(error) => error == crate::spec::readers::eof_error(
                    old(self).position, 2, (old(self).bytes@.len() - old(self).position) as usize,
                ),
                Ok(_) => true,
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::question_mark)]
    pub(crate) fn read_u16(&mut self) -> Result<u16, MalformedError> {
        let octets = match self.read_bytes(2) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        let byte0 = octets[0];
        let byte1 = octets[1];
        let value = (u16::from(byte0) << 8) | u16::from(byte1);
        Ok(value)
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires old(self).position <= old(self).bytes@.len(),
        ensures
            final(self).bytes@ == old(self).bytes@,
            match out {
                Ok(bytes) =>
                    length <= old(self).bytes@.len() - old(self).position
                    && final(self).position == old(self).position + length
                    && bytes@ == old(self).bytes@.subrange(
                        old(self).position as int,
                        (old(self).position + length) as int,
                    ),
                Err(_) =>
                    final(self).position == old(self).position
                    && length > old(self).bytes@.len() - old(self).position,
            },
            match out {
                Err(error) => error == crate::spec::readers::eof_error(
                    old(self).position, length, (old(self).bytes@.len() - old(self).position) as usize,
                ),
                Ok(_) => true,
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    pub(crate) fn read_bytes(&mut self, length: usize) -> Result<&'a [u8], MalformedError> {
        let remaining = self.remaining();
        if length > remaining {
            return Err(MalformedError::UnexpectedEof {
                offset: self.position as u64,
                needed: length as u64,
                remaining: remaining as u64,
            });
        }
        #[cfg(verus_keep_ghost)]
        production_read_bytes_addition!(self, length);
        let end = self.position + length;
        let result = &self.bytes[self.position..end];
        self.position = end;
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(crate) struct BitReader<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) bit_position: usize,
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out.bytes@ == bytes@, out.bit_position == 0,
))]
pub(crate) const fn bit_reader(bytes: &[u8]) -> BitReader<'_> {
    BitReader {
        bytes,
        bit_position: 0,
    }
}

impl BitReader<'_> {
    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        ensures out == self.bit_position,
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(dead_code)]
    pub(crate) const fn bit_position(&self) -> usize {
        self.bit_position
    }

    #[cfg_attr(verus_keep_ghost, verus_spec(out =>
        requires
            old(self).bit_position <= old(self).bytes@.len() * 8,
            old(self).bytes@.len() <= usize::MAX / 8,
        ensures
            final(self).bytes@ == old(self).bytes@,
            match out {
                Ok(value) =>
                    count <= 32
                    && count as nat <= old(self).bytes@.len() * 8 - old(self).bit_position
                    && final(self).bit_position == old(self).bit_position + count
                    && value as nat == crate::spec::readers::bits_value(
                        old(self).bytes@,
                        old(self).bit_position as nat,
                        count as nat,
                    ),
                Err(_) => final(self).bit_position == old(self).bit_position,
            },
            match out {
                Err(error) => Some(error) == crate::spec::readers::bit_read_error(
                    old(self).bytes@, old(self).bit_position, count,
                ),
                Ok(_) => true,
            },
    ))]
    #[cfg_attr(verus_keep_ghost, allow(unused, verus_impl_method_marker))]
    #[allow(clippy::too_many_lines)]
    pub(crate) fn read_bits(&mut self, count: u8) -> Result<u32, MalformedError> {
        if count > 32 {
            return Err(MalformedError::InvalidBitCount(count));
        }
        let total_bits = self
            .bytes
            .len()
            .checked_mul(8)
            .ok_or(MalformedError::IntegerOverflow)?;
        let remaining = total_bits - self.bit_position;
        let count_usize = usize::from(count);
        if count_usize > remaining {
            return Err(MalformedError::UnexpectedEof {
                offset: self.bit_position as u64,
                needed: u64::from(count),
                remaining: remaining as u64,
            });
        }

        let mut value = 0_u32;
        #[cfg(verus_keep_ghost)]
        let start = self.bit_position;
        let end = self
            .bit_position
            .checked_add(count_usize)
            .ok_or(MalformedError::IntegerOverflow)?;
        let mut position = self.bit_position;
        if count >= 8 {
            let shift = position % 8;
            #[cfg_attr(verus_keep_ghost, verus_spec(
                invariant
                    start == old(self).bit_position,
                    start <= position <= end,
                    end == start + count as usize,
                    end <= self.bytes@.len() * 8,
                    count <= 32,
                    shift == position % 8,
                    value as nat == crate::spec::readers::bits_value(
                        self.bytes@, start as nat, (position - start) as nat,
                    ),
                decreases end - position,
            ))]
            while end - position >= 8 {
                let first = self.bytes[position / 8];
                let byte = if shift == 0 {
                    first
                } else {
                    let last = self.bytes[position / 8 + 1];
                    (first << shift) | (last >> (8 - shift))
                };
                #[cfg(verus_keep_ghost)]
                proof! {
                    crate::reader_properties::unaligned_byte_append(
                        self.bytes@, start as nat, (position - start) as nat, value, byte,
                    );
                }
                value = (value << 8) | u32::from(byte);
                position += 8;
            }
        }
        #[cfg_attr(verus_keep_ghost, verus_spec(
            invariant
                start == old(self).bit_position,
                count <= 32,
                start <= position <= end,
                end == start + count as usize,
                end <= self.bytes@.len() * 8,
                value as nat == crate::spec::readers::bits_value(
                    self.bytes@,
                    start as nat,
                    (position - start) as nat,
                ),
            decreases end - position,
        ))]
        while position < end {
            #[cfg(verus_keep_ghost)]
            production_read_bits_before!(previous_value, previous_position, value, position);
            let byte = self.bytes[position / 8];
            let shift = 7 - (position % 8);
            let bit = u32::from((byte >> shift) & 1);
            #[cfg(verus_keep_ghost)]
            proof! {
                crate::reader_properties::bits_value_bound(
                    self.bytes@, start as nat, (position - start) as nat,
                );
                crate::reader_properties::bits_limit_monotone((position - start) as nat, 31);
                assert(crate::reader_properties::bits_limit(31) == 0x8000_0000) by (compute);
                assert(value <= u32::MAX / 2);
                assert(((byte >> shift) & 1) <= 1) by (bit_vector);
                assert(bit <= 1);
            }
            let Some(doubled) = value.checked_mul(2) else {
                return Err(MalformedError::IntegerOverflow);
            };
            let Some(next_value) = doubled.checked_add(bit) else {
                return Err(MalformedError::IntegerOverflow);
            };
            value = next_value;
            position += 1;
            #[cfg(verus_keep_ghost)]
            production_read_bits_after!(
                previous_value,
                previous_position,
                value,
                position,
                byte,
                bit,
                shift,
                self,
                start
            );
        }
        self.bit_position = end;
        Ok(value)
    }
}

#[cfg(test)]
mod tests;
