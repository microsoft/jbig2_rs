use super::{
    allocation_size, build_document_info, enforce_input_work, enforce_segment_limit,
    enforce_stream_limit, enforce_work_limit, inspect_file_header, parse_end_of_stripe,
    parse_extension, parse_page_information, parse_profiles, parse_segment_header, require_empty,
    ByteReader, DecodeError, DocumentInfo, GlobalSegments, HuffmanError, HuffmanTable, Limits,
    MalformedError, Organization, ParsedData, ParsedSegment, SegmentHeader, SegmentType,
    UnsupportedFeature,
};

#[cfg(verus_keep_ghost)]
use super::document_spec;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(result) =>
            document_spec::standalone_metadata(input@, &result.0)
            && document_spec::document_segment_count(&result.0) as nat == result.1@.len()
            && document_spec::document_pages(&result.0).len()
                == document_spec::page_information_count(result.1@, result.1@.len())
            && match crate::file_header::file_header_spec::organization(input@) {
                Organization::Sequential => document_spec::sequential_container_fields(
                    input@.subrange(
                        crate::file_header::file_header_spec::byte_length(input@) as int,
                        input@.len() as int,
                    ),
                    result.1@, true,
                ),
                Organization::RandomAccess => document_spec::random_access_container_fields(
                    input@.subrange(
                        crate::file_header::file_header_spec::byte_length(input@) as int,
                        input@.len() as int,
                    ),
                    result.1@,
                ),
            },
        Err(_) => true,
    },
))]
pub(crate) fn parse_standalone_document<'a>(
    input: &'a [u8],
    limits: &Limits,
) -> Result<(DocumentInfo, Vec<ParsedSegment<'a>>), DecodeError> {
    enforce_stream_limit(input.len(), limits)?;
    enforce_input_work(input.len(), limits)?;
    let file_header = inspect_file_header(input)?;
    if file_header.uses_colour_regions() {
        return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
    }
    let header_length = file_header.byte_length();
    if header_length > input.len() {
        return Err(MalformedError::IntegerOverflow.into());
    }
    let body = &input[header_length..];
    let segments = match file_header.organization() {
        Organization::Sequential => parse_sequential(body, limits, true)?,
        Organization::RandomAccess => parse_random_access(body, limits)?,
    };
    enforce_work_limit(input.len(), &segments, limits)?;
    let info = build_document_info(
        file_header.organization(),
        file_header.page_count(),
        &segments,
        limits,
        None,
        false,
    )?;
    Ok((info, segments))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(result) =>
            document_spec::embedded_metadata(&result.0)
            && document_spec::document_segment_count(&result.0) as nat == result.1@.len()
            && document_spec::document_pages(&result.0).len()
                == document_spec::page_information_count(result.1@, result.1@.len())
            && document_spec::sequential_container_fields(input@, result.1@, false),
        Err(_) => true,
    },
))]
pub(crate) fn parse_embedded_document<'a>(
    input: &'a [u8],
    limits: &Limits,
    globals: Option<&GlobalSegments>,
) -> Result<(DocumentInfo, Vec<ParsedSegment<'a>>), DecodeError> {
    enforce_stream_limit(input.len(), limits)?;
    enforce_input_work(input.len(), limits)?;
    let segments = parse_sequential(input, limits, false)?;
    enforce_work_limit(input.len(), &segments, limits)?;
    let info = build_document_info(
        Organization::Sequential,
        None,
        &segments,
        limits,
        globals,
        true,
    )?;
    Ok((info, segments))
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(segments) => document_spec::sequential_container_fields(
            input@, segments@, require_local_references,
        ),
        Err(_) => true,
    },
))]
pub(super) fn parse_sequential<'a>(
    input: &'a [u8],
    limits: &Limits,
    require_local_references: bool,
) -> Result<Vec<ParsedSegment<'a>>, DecodeError> {
    let mut reader = ByteReader::new(input);
    let mut segments = Vec::new();
    let mut previous_number = None;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost mut windows = Seq::<(int, int, int)>::empty(); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            reader.position <= reader.bytes@.len(),
            reader.bytes@ == input@,
            document_spec::sequential_windows(
                input@, segments@, windows, reader.position as int, require_local_references,
            ),
            document_spec::segment_metadata(segments@),
            match previous_number {
                Some(number) => segments@.len() > 0 && segments@.last().header.number == number,
                None => segments@.len() == 0,
            },
        ensures
            reader.bytes@ == input@,
            reader.position == input@.len(),
            document_spec::sequential_windows(
                input@, segments@, windows, reader.position as int, require_local_references,
            ),
            document_spec::segment_metadata(segments@),
        decreases reader.bytes@.len() - reader.position,
    ))]
    while reader.remaining() != 0 {
        enforce_segment_limit(segments.len(), limits)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost header_start = reader.position as int; }
        let header = parse_segment_header(&mut reader, limits)?;
        if let Some(previous) = previous_number {
            if require_local_references && header.number <= previous {
                return Err(MalformedError::NonIncreasingSegmentNumber {
                    previous,
                    current: header.number,
                }
                .into());
            }
        }
        previous_number = Some(header.number);
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost payload_start = reader.position as int; }
        let payload = read_payload(&mut reader, &header)?;
        let is_end = matches!(header.segment_type, SegmentType::EndOfFile);
        let data = parse_known_segment_data(&header, payload, limits)?;
        let new_len = segments
            .len()
            .checked_add(1)
            .ok_or(MalformedError::IntegerOverflow)?;
        let segment_allocation = allocation_size::<ParsedSegment<'_>>(new_len)?;
        if segments.try_reserve(1).is_err() {
            return Err(DecodeError::Allocation {
                requested_bytes: segment_allocation,
            });
        }
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost old_segments = segments@; }
        segments.push(ParsedSegment {
            header,
            payload,
            data,
        });
        #[cfg(verus_keep_ghost)]
        production_sequential_window_append!(
            input,
            segments,
            old_segments,
            windows,
            header_start,
            payload_start,
            reader,
            require_local_references
        );
        if is_end {
            if reader.remaining() != 0 {
                return Err(MalformedError::TrailingData(reader.remaining() as u64).into());
            }
            break;
        }
    }
    #[cfg(verus_keep_ghost)]
    production_sequential_complete!(input, segments, windows, reader, require_local_references);
    Ok(segments)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(segments) => document_spec::random_access_container_fields(input@, segments@),
        Err(_) => true,
    },
))]
pub(super) fn parse_random_access<'a>(
    input: &'a [u8],
    limits: &Limits,
) -> Result<Vec<ParsedSegment<'a>>, DecodeError> {
    let mut reader = ByteReader::new(input);
    let mut headers = Vec::new();
    let mut previous_number = None;
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost mut header_windows = Seq::<(int, int)>::empty(); }
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            reader.position <= reader.bytes@.len() && reader.bytes@ == input@,
            document_spec::random_header_windows(input@, headers@, header_windows, reader.position as int),
            match previous_number {
                Some(number) => headers@.len() > 0 && headers@.last().number == number,
                None => headers@.len() == 0,
            },
        invariant_except_break
            Seq::<SegmentHeader>::len(headers@) > 0 ==> headers@.last().segment_type != SegmentType::EndOfFile,
        ensures
            reader.position <= reader.bytes@.len() && reader.bytes@ == input@,
            document_spec::random_header_windows(input@, headers@, header_windows, reader.position as int),
            headers@.len() > 0 && headers@.last().segment_type == SegmentType::EndOfFile,
        decreases reader.bytes@.len() - reader.position,
    ))]
    loop {
        if reader.remaining() == 0 {
            return Err(MalformedError::MissingEndOfFile.into());
        }
        enforce_segment_limit(headers.len(), limits)?;
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost header_before = (reader.position as int, headers@); }
        let header = parse_segment_header(&mut reader, limits)?;
        if let Some(previous) = previous_number {
            if header.number <= previous {
                return Err(MalformedError::NonIncreasingSegmentNumber {
                    previous,
                    current: header.number,
                }
                .into());
            }
        }
        previous_number = Some(header.number);
        let is_end = matches!(header.segment_type, SegmentType::EndOfFile);
        let requested_bytes = allocation_size::<SegmentHeader>(1)?;
        if headers.try_reserve(1).is_err() {
            return Err(DecodeError::Allocation { requested_bytes });
        }
        headers.push(header);
        #[cfg(verus_keep_ghost)]
        production_random_header_append!(input, headers, header_windows, header_before, reader);
        if is_end {
            break;
        }
    }

    let mut segments = Vec::new();
    let segment_allocation = allocation_size::<ParsedSegment<'_>>(headers.len())?;
    if segments.try_reserve(headers.len()).is_err() {
        return Err(DecodeError::Allocation {
            requested_bytes: segment_allocation,
        });
    }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost header_seq = headers@; }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost header_end = reader.position as int; }
    #[cfg(verus_keep_ghost)]
    proof_decl! { let ghost mut spans = Seq::<(int, int)>::empty(); }
    #[cfg_attr(verus_keep_ghost, verus_spec(header_iter =>
        invariant
            reader.position <= reader.bytes@.len() && reader.bytes@ == input@,
            header_iter.seq() == header_seq,
            segments@.len() == header_iter.index(),
            document_spec::random_header_windows(input@, header_seq, header_windows, header_end),
            header_seq.len() > 0 && header_seq.last().segment_type == SegmentType::EndOfFile,
            document_spec::random_payload_windows(input@, header_seq, header_end, segments@, spans, reader.position as int),
            document_spec::segment_metadata(segments@),
    ))]
    for header in headers {
        #[cfg(verus_keep_ghost)]
        proof_decl! { let ghost before = (reader.position as int, segments@); }
        let payload = read_payload(&mut reader, &header)?;
        let data = parse_known_segment_data(&header, payload, limits)?;
        segments.push(ParsedSegment {
            header,
            payload,
            data,
        });
        #[cfg(verus_keep_ghost)]
        production_random_payload!(header_seq, header_end, segments, spans, before, reader);
    }
    if reader.remaining() != 0 {
        return Err(MalformedError::TrailingData(reader.remaining() as u64).into());
    }
    #[cfg(verus_keep_ghost)]
    production_random_access_complete!(
        input,
        header_seq,
        header_windows,
        header_end,
        segments,
        spans
    );
    Ok(segments)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires old(reader).position <= old(reader).bytes@.len(),
    ensures
        final(reader).bytes@ == old(reader).bytes@,
        final(reader).position <= final(reader).bytes@.len(),
        final(reader).position >= old(reader).position,
        match out {
            Ok(payload) => document_spec::payload_window(
                header, old(reader).bytes@, old(reader).position as int,
                final(reader).position as int, payload@,
            ),
            Err(_) => true,
        },
))]
pub(super) fn read_payload<'a>(
    reader: &mut ByteReader<'a>,
    header: &SegmentHeader,
) -> Result<&'a [u8], DecodeError> {
    if let Some(length) = header.data_length {
        let Ok(length) = usize::try_from(length) else {
            return Err(MalformedError::IntegerOverflow.into());
        };
        return Ok(reader.read_bytes(length)?);
    }
    let length = find_unknown_generic_length(reader.remaining_bytes())?;
    Ok(reader.read_bytes(length)?)
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn find_unknown_generic_length(bytes: &[u8]) -> Result<usize, DecodeError> {
    const GENERIC_FLAGS_INDEX: usize = 17;
    const SEARCH_START: usize = 18;
    const TERMINAL_BYTES: usize = 6;
    if bytes.len() < SEARCH_START + TERMINAL_BYTES {
        return Err(MalformedError::MissingGenericRegionTerminator.into());
    }
    let declared_height = read_be_u32_at(bytes, 4);
    let marker = if bytes[GENERIC_FLAGS_INDEX] & 0x01 != 0 {
        [0x00, 0x00]
    } else {
        [0xFF, 0xAC]
    };
    let final_start = bytes
        .len()
        .checked_sub(TERMINAL_BYTES)
        .ok_or(MalformedError::IntegerOverflow)?;
    let mut offset = SEARCH_START;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            bytes@.len() >= SEARCH_START + TERMINAL_BYTES,
            final_start == bytes@.len() - TERMINAL_BYTES,
            SEARCH_START <= offset <= final_start + 1,
        decreases final_start + 1 - offset,
    ))]
    while offset <= final_start {
        if bytes[offset] != marker[0] || bytes[offset + 1] != marker[1] {
            offset += 1;
            continue;
        }
        let row_count = read_be_u32_at(bytes, offset + 2);
        if row_count > declared_height {
            return Err(MalformedError::GenericRegionRowCount {
                declared: declared_height,
                actual: row_count,
            }
            .into());
        }
        return offset
            .checked_add(TERMINAL_BYTES)
            .ok_or(MalformedError::IntegerOverflow.into());
    }
    Err(MalformedError::MissingGenericRegionTerminator.into())
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires offset + 4 <= bytes@.len(),
    ensures out
        == ((bytes@[offset as int] as u32) << 24)
            | ((bytes@[(offset + 1) as int] as u32) << 16)
            | ((bytes@[(offset + 2) as int] as u32) << 8)
            | bytes@[(offset + 3) as int] as u32,
))]
pub(super) fn read_be_u32_at(bytes: &[u8], offset: usize) -> u32 {
    (u32::from(bytes[offset]) << 24)
        | (u32::from(bytes[offset + 1]) << 16)
        | (u32::from(bytes[offset + 2]) << 8)
        | u32::from(bytes[offset + 3])
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(data) => document_spec::known_segment_data_fields(header.segment_type, payload@, data),
        Err(_) => true,
    },
))]
pub(super) fn parse_known_segment_data(
    header: &SegmentHeader,
    payload: &[u8],
    limits: &Limits,
) -> Result<ParsedData, DecodeError> {
    let data = match header.segment_type {
        SegmentType::PageInformation => ParsedData::Page(parse_page_information(payload, limits)?),
        SegmentType::EndOfStripe => ParsedData::EndOfStripe(parse_end_of_stripe(payload)?),
        SegmentType::EndOfPage => {
            require_empty(49, payload)?;
            ParsedData::Other
        }
        SegmentType::EndOfFile => {
            require_empty(51, payload)?;
            ParsedData::Other
        }
        SegmentType::Profiles => ParsedData::Profiles(parse_profiles(payload)?),
        SegmentType::Tables => {
            if let Err(error) = HuffmanTable::from_encoded(payload, limits) {
                let error = match error {
                    HuffmanError::Decode(error) => error,
                    HuffmanError::InvalidTable(kind) => {
                        DecodeError::Malformed(MalformedError::InvalidHuffman(kind))
                    }
                };
                return Err(error);
            }
            ParsedData::Other
        }
        SegmentType::Extension => {
            parse_extension(payload)?;
            ParsedData::Other
        }
        SegmentType::ColourPalette => {
            return Err(DecodeError::Unsupported(UnsupportedFeature::Colour));
        }
        _ => ParsedData::Other,
    };
    #[cfg(verus_keep_ghost)]
    production_known_segment_data_fields!();
    Ok(data)
}
