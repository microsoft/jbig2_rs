use super::*;
use crate::huffman::{bit_reader, HuffmanLine, StandardHuffmanTable};
use crate::{DecodeError, Limits};

fn same(
    descriptor: StandardADescriptor,
    owned: &HuffmanTable,
    bytes: &[u8],
    start: usize,
    final_strip: bool,
) {
    let mut actual = bit_reader(bytes);
    actual.bit_position = start;
    let mut reference = actual;
    let (left, right) = if final_strip {
        (
            descriptor.decode_final_strip_value(&mut actual),
            owned.decode_final_strip_value(&mut reference),
        )
    } else {
        (
            descriptor.decode(&mut actual).map(Some),
            owned.decode(&mut reference).map(Some),
        )
    };
    assert_eq!(format!("{left:?}"), format!("{right:?}"));
    assert_eq!(actual.bit_position, reference.bit_position);
    assert_eq!(actual.bytes.as_ptr(), bytes.as_ptr());
    assert_eq!(actual.bytes, reference.bytes);
}

#[test]
fn all_prefix_offsets_and_suffix_eof_states_match_both_owned_constructors() {
    let limits = Limits::default();
    let descriptor = StandardADescriptor::new(&limits).unwrap();
    let standard = HuffmanTable::standard(StandardHuffmanTable::A, &limits).unwrap();
    let raw_table = HuffmanTable::new(&TABLE_A, &limits).unwrap();
    for seed in 0..=u8::MAX {
        let buffer = [seed, !seed, 0xff, 0x80, 0, 0x55];
        for end in 0..=buffer.len() {
            for start in 0..=end * 8 {
                for final_strip in [false, true] {
                    same(descriptor, &standard, &buffer[..end], start, final_strip);
                    same(descriptor, &raw_table, &buffer[..end], start, final_strip);
                }
            }
        }
    }
}

#[test]
fn upper_range_overflow_keeps_the_consumed_32_bit_suffix() {
    let limits = Limits::default();
    let owned = HuffmanTable::standard(StandardHuffmanTable::A, &limits).unwrap();
    let descriptor = StandardADescriptor::new(&limits).unwrap();
    let last = u32::try_from(i32::MAX - 65_808).unwrap();
    for offset in [0, last, last + 1, u32::MAX] {
        let word = ((7_u64 << 32) | u64::from(offset)) << 5;
        let bytes = word.to_be_bytes();
        same(descriptor, &owned, &bytes[3..], 0, false);
        let mut reader = bit_reader(&bytes[3..]);
        let decoded = descriptor.decode(&mut reader);
        if offset > last {
            assert!(matches!(
                decoded,
                Err(HuffmanError::Decode(DecodeError::Malformed(
                    MalformedError::IntegerOverflow,
                )))
            ));
        } else {
            assert_eq!(
                decoded.ok(),
                Some(HuffmanValue::Integer(
                    65_808 + i32::try_from(offset).unwrap()
                )),
            );
        }
        assert_eq!(reader.bit_position, 35);
    }
}

#[test]
fn descriptor_preserves_four_logical_entries_but_removes_the_owned_memory_floor() {
    let mut limits = Limits {
        max_symbols: 3,
        max_decoder_managed_memory_bytes: 0,
        ..Limits::default()
    };
    assert!(matches!(
        StandardADescriptor::new(&limits),
        Err(HuffmanError::Decode(DecodeError::LimitExceeded(
            LimitExceeded {
                kind: LimitKind::Symbols,
                limit: 3,
                actual: 4,
            }
        )))
    ));
    limits.max_symbols = 4;
    StandardADescriptor::new(&limits).unwrap();
    let owned_charge = crate::huffman::table_memory(4, false).unwrap();
    for memory in [0, owned_charge - 1] {
        limits.max_decoder_managed_memory_bytes = memory;
        StandardADescriptor::new(&limits).unwrap();
        assert!(matches!(
            HuffmanTable::standard(StandardHuffmanTable::A, &limits),
            Err(HuffmanError::Decode(DecodeError::LimitExceeded(LimitExceeded {
                kind: LimitKind::Memory, limit, actual,
            }))) if limit == memory && actual == owned_charge
        ));
    }
    limits.max_decoder_managed_memory_bytes = owned_charge;
    HuffmanTable::standard(StandardHuffmanTable::A, &limits).unwrap();
    eprintln!(
        "A owned charge={owned_charge}; descriptor={}, cache slot={}, view={}, optional view={}",
        size_of::<StandardADescriptor>(),
        size_of::<Option<StandardADescriptor>>(),
        size_of::<CodecHuffmanTable<'_>>(),
        size_of::<Option<CodecHuffmanTable<'_>>>(),
    );
}

#[test]
fn long_custom_view_stays_on_the_existing_backend() {
    let limits = Limits::default();
    let owned = HuffmanTable::new(&[HuffmanLine::normal(23, 65, 0)], &limits).unwrap();
    let view = CodecHuffmanTable::General(&owned);
    assert!(core::ptr::eq(
        view.general().unwrap(),
        core::ptr::from_ref(&owned),
    ));
    for end in 0..=9 {
        let buffer = [0_u8; 9];
        let mut actual = bit_reader(&buffer[..end]);
        let mut reference = actual;
        assert_eq!(
            format!("{:?}", view.decode(&mut actual)),
            format!("{:?}", owned.decode(&mut reference)),
        );
        assert_eq!(actual.bit_position, reference.bit_position);
    }
}
