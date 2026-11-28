use crate::reader::ByteReader;
use crate::segment_header::SegmentHeader;
use crate::{DecodeError, LimitKind, Limits, MalformedError, UnsupportedFeature};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_verify)]
const DEFAULT: [[u8; 3]; 32] = [
    [0, 0, 0],
    [128, 128, 128],
    [192, 192, 192],
    [255, 255, 255],
    [255, 0, 0],
    [0, 255, 0],
    [0, 0, 255],
    [255, 255, 0],
    [0, 255, 255],
    [255, 0, 255],
    [128, 0, 0],
    [0, 128, 0],
    [0, 0, 128],
    [128, 128, 0],
    [0, 128, 128],
    [128, 0, 128],
    [255, 165, 0],
    [204, 204, 0],
    [153, 0, 0],
    [0, 204, 0],
    [0, 153, 0],
    [204, 204, 0],
    [153, 153, 0],
    [102, 0, 0],
    [0, 0, 204],
    [0, 0, 153],
    [204, 0, 204],
    [153, 0, 153],
    [0, 204, 204],
    [0, 153, 153],
    [102, 102, 102],
    [153, 153, 153],
];

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) struct Palette {
    pub(super) number: u32,
    pub(super) page: u32,
    pub(super) live: bool,
    pub(super) colours: Vec<[u8; 3]>,
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(palette) => super::colour_spec::palette_contents(payload@, palette.colours@)
            && palette.number == header.number && palette.page == header.page_association,
        Err(_) => true,
    },
))]
#[allow(clippy::too_many_lines)]
pub(super) fn parse(
    header: &SegmentHeader,
    payload: &[u8],
    limits: &Limits,
) -> Result<Palette, DecodeError> {
    if header.retention_flags.values.is_empty() {
        return Err(MalformedError::InvalidSegmentReferences(header.number).into());
    }
    let mut reader = ByteReader::new(payload);
    let flags = reader.read_u8()?;
    if flags & 0xe0 != 0 || (flags >> 1) & 15 > 2 {
        return Err(MalformedError::InvalidPageState("reserved colour palette flags").into());
    }
    if flags & 1 != 0 {
        return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
    }
    let components = reader.read_u8()?;
    let component_length = reader.read_u8()?;
    let count = reader.read_u32()?;
    if components != 3 {
        return Err(
            MalformedError::InvalidPageState("invalid RGB palette component layout").into(),
        );
    }
    let component_bytes = match component_length {
        1 => 1_usize,
        2 => 2,
        4 => 4,
        _ => {
            return Err(
                MalformedError::InvalidPageState("invalid RGB palette component layout").into(),
            )
        }
    };
    let count = usize::try_from(count).map_err(|_| MalformedError::IntegerOverflow)?;
    let component_count = count
        .checked_mul(3)
        .ok_or(MalformedError::IntegerOverflow)?;
    let encoded = component_count
        .checked_mul(component_bytes)
        .ok_or(MalformedError::IntegerOverflow)?;
    if reader.remaining() != encoded {
        return Err(MalformedError::InvalidPageState(
            "colour palette length does not match its count",
        )
        .into());
    }
    if component_length != 1 || (flags >> 1) & 15 == 2 {
        return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
    }
    let requested = u64::try_from(count)
        .map_err(|_| MalformedError::IntegerOverflow)?
        .checked_mul(3)
        .ok_or(MalformedError::IntegerOverflow)?;
    super::document::limit(
        LimitKind::Memory,
        requested,
        limits.max_decoder_managed_memory_bytes,
    )?;
    super::document::limit(
        LimitKind::RetainedState,
        requested,
        limits.max_retained_state_bytes,
    )?;
    let mut colours: Vec<[u8; 3]> = Vec::new();
    colours
        .try_reserve(count)
        .map_err(|_| DecodeError::Allocation {
            requested_bytes: requested,
        })?;
    let capacity = u64::try_from(colours.capacity())
        .map_err(|_| MalformedError::IntegerOverflow)?
        .checked_mul(3)
        .ok_or(MalformedError::IntegerOverflow)?;
    super::document::limit(
        LimitKind::Memory,
        capacity,
        limits.max_decoder_managed_memory_bytes,
    )?;
    super::document::limit(
        LimitKind::RetainedState,
        capacity,
        limits.max_retained_state_bytes,
    )?;
    let mut index = 0_usize;
    #[cfg(verus_keep_ghost)]
    proof! {
        assert(reader.position == 7);
        assert(encoded as int == count as int * 3);
        assert(payload@.len() == 7 + count * 3);
    }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= count, reader.position <= reader.bytes@.len(),
            reader.bytes@ == payload@, reader.position == 7 + index * 3,
            payload@.len() == 7 + count * 3, colours@.len() == index,
            count == crate::spec::segments::be_u32_at(payload@, 3),
            forall|done:int, channel:int| 0 <= done < index && 0 <= channel < 3 ==>
                colours@[done]@[channel] == payload@[7 + done * 3 + channel],
        decreases count - index,
    ))]
    while index < count {
        let red = reader.read_u8()?;
        let green = reader.read_u8()?;
        let blue = reader.read_u8()?;
        colours.push([red, green, blue]);
        index += 1;
    }
    Ok(Palette {
        number: header.number,
        page: header.page_association,
        live: header.retention_flags.retain_this_segment(),
        colours,
    })
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Some(index) => index < palettes@.len() && palettes@[index as int].number == number,
        None => true,
    },
))]
pub(super) fn find(palettes: &[Palette], number: u32) -> Option<usize> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant index <= palettes@.len(),
        decreases palettes@.len() - index,
    ))]
    while index < palettes.len() {
        if palettes[index].number == number {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out.is_ok() ==> forall|i: int| 0 <= i < header.referred_segments@.len() ==> (
        header.referred_segments@[i] < header.number
            && exists|j: int| 0 <= j < palettes@.len()
                && palettes@[j].number == header.referred_segments@[i]
                && palettes@[j].live
                && (palettes@[j].page == 0 || palettes@[j].page == header.page_association)
    ),
))]
pub(super) fn validate_references(
    header: &SegmentHeader,
    palettes: &[Palette],
) -> Result<(), DecodeError> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= header.referred_segments@.len(),
            forall|i: int| 0 <= i < index ==> (
                header.referred_segments@[i] < header.number
                    && exists|j: int| 0 <= j < palettes@.len()
                        && palettes@[j].number == header.referred_segments@[i]
                        && palettes@[j].live
                        && (palettes@[j].page == 0 || palettes@[j].page == header.page_association)
            ),
        decreases header.referred_segments@.len() - index,
    ))]
    while index < header.referred_segments.len() {
        let number = header.referred_segments[index];
        if number >= header.number {
            return Err(MalformedError::InvalidSegmentReferences(header.number).into());
        }
        let palette_index = find(palettes, number)
            .ok_or(MalformedError::InvalidSegmentReferences(header.number))?;
        let palette = &palettes[palette_index];
        if !palette.live || palette.page != 0 && palette.page != header.page_association {
            return Err(MalformedError::InvalidSegmentReferences(header.number).into());
        }
        index += 1;
    }
    Ok(())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        out.is_ok() ==> forall|i: int| 0 <= i < header.referred_segments@.len() ==>
            header.referred_segments@[i] < header.number,
))]
pub(super) fn resolve(
    header: &SegmentHeader,
    palettes: &[Palette],
    id: u32,
) -> Result<[u8; 3], DecodeError> {
    validate_references(header, palettes)?;
    let mut remaining = usize::try_from(id).map_err(|_| MalformedError::IntegerOverflow)?;
    if remaining < DEFAULT.len() {
        return Ok(DEFAULT[remaining]);
    }
    remaining -= DEFAULT.len();
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= header.referred_segments@.len(),
            forall|i: int| 0 <= i < header.referred_segments@.len() ==>
                header.referred_segments@[i] < header.number,
        decreases header.referred_segments@.len() - index,
    ))]
    while index < header.referred_segments.len() {
        let palette_index = find(palettes, header.referred_segments[index])
            .ok_or(MalformedError::InvalidSegmentReferences(header.number))?;
        let colours = &palettes[palette_index].colours;
        if remaining < colours.len() {
            return Ok(colours[remaining]);
        }
        remaining -= colours.len();
        index += 1;
    }
    Err(MalformedError::InvalidPageState("colour palette index out of range").into())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures
        final(palettes)@.len() == old(palettes)@.len(),
        forall|j: int| 0 <= j < old(palettes)@.len() ==> (
            final(palettes)@[j].number == old(palettes)@[j].number
                && final(palettes)@[j].page == old(palettes)@[j].page
                && final(palettes)@[j].colours@ == old(palettes)@[j].colours@
                && (final(palettes)@[j].live ==> old(palettes)@[j].live)
        ),
))]
pub(super) fn release(header: &SegmentHeader, palettes: &mut [Palette]) -> Result<(), DecodeError> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= header.referred_segments@.len(),
            palettes@.len() == old(palettes)@.len(),
            forall|j: int| 0 <= j < old(palettes)@.len() ==> (
                palettes@[j].number == old(palettes)@[j].number
                    && palettes@[j].page == old(palettes)@[j].page
                    && palettes@[j].colours@ == old(palettes)@[j].colours@
                    && (palettes@[j].live ==> old(palettes)@[j].live)
            ),
        decreases header.referred_segments@.len() - index,
    ))]
    while index < header.referred_segments.len() {
        if !header
            .retention_flags
            .retain_referred_segment(index)
            .ok_or(MalformedError::InvalidSegmentReferences(header.number))?
        {
            let palette_index = find(palettes, header.referred_segments[index])
                .ok_or(MalformedError::InvalidSegmentReferences(header.number))?;
            palettes[palette_index].live = false;
        }
        index += 1;
    }
    Ok(())
}
