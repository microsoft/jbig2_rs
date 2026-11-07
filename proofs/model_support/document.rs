// Source-only author navigation; named purposes are not normative approval.
// Guide, official acquisition and source identity: ../../specs/README.md
// All requirement links and inherited locators: ../../validation/traceability/requirements.json
// REQUIREMENT CONTEXT is not a direct normative equation, including for proof/policy helpers.

// Included in document_spec to preserve closed-model and field visibility.
verus! {

// Model navigation: spec::sequential_windows [proof-support]
// Purpose: Window-sequence bookkeeping for parsed headers and payloads.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn sequential_windows(
    bytes: Seq<u8>,
    segments: Seq<crate::document::ParsedSegment<'_>>,
    windows: Seq<(int, int, int)>,
    end: int,
    ordered: bool,
) -> bool {
    &&& windows.len() == segments.len()
    &&& 0 <= end <= bytes.len()
    &&& if windows.len() == 0 { end == 0 } else { windows.last().2 == end }
    &&& forall|index: int| 0 <= index < segments.len() ==> {
        let window = #[trigger] windows[index];
        let segment = segments[index];
        &&& 0 <= window.0 < window.1 <= window.2 <= end
        &&& window.0 == if index == 0 { 0 } else { windows[index - 1].2 }
        &&& crate::spec::segments::segment_header_fields(
            &segment.header, bytes, window.0, window.1,
        )
        &&& payload_window(&segment.header, bytes, window.1, window.2, segment.payload@)
        &&& (ordered && index > 0 ==> segments[index - 1].header.number < segment.header.number)
    }
}

// Model navigation: spec::random_header_windows [proof-support]
// Purpose: Window-sequence bookkeeping for parsed headers and payloads.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn random_header_windows(
    bytes: Seq<u8>,
    headers: Seq<crate::segment_header::SegmentHeader>,
    windows: Seq<(int, int)>,
    end: int,
) -> bool {
    &&& headers.len() == windows.len()
    &&& 0 <= end <= bytes.len()
    &&& if headers.len() == 0 { end == 0 } else { windows.last().1 == end }
    &&& forall|index: int| 0 <= index < headers.len() ==> {
        let window = #[trigger] windows[index];
        &&& 0 <= window.0 < window.1 <= end
        &&& window.0 == if index == 0 { 0 } else { windows[index - 1].1 }
        &&& crate::spec::segments::segment_header_fields(
            &headers[index], bytes, window.0, window.1,
        )
        &&& (index > 0 ==> headers[index - 1].number < headers[index].number)
        &&& (index + 1 < headers.len() ==>
            headers[index].segment_type != crate::segment_header::SegmentType::EndOfFile)
    }
}

// Model navigation: spec::random_payload_windows [proof-support]
// Purpose: Window-sequence bookkeeping for parsed headers and payloads.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn random_payload_windows(
    bytes: Seq<u8>,
    headers: Seq<crate::segment_header::SegmentHeader>,
    header_end: int,
    segments: Seq<crate::document::ParsedSegment<'_>>,
    windows: Seq<(int, int)>,
    end: int,
) -> bool {
    &&& segments.len() <= headers.len()
    &&& segments.len() == windows.len()
    &&& 0 <= header_end <= end <= bytes.len()
    &&& if segments.len() == 0 { end == header_end } else { windows.last().1 == end }
    &&& forall|index: int| 0 <= index < segments.len() ==> {
        let segment = #[trigger] segments[index];
        let window = windows[index];
        &&& segment.header == headers[index]
        &&& header_end <= window.0 <= window.1 <= end
        &&& window.0 == if index == 0 { header_end } else { windows[index - 1].1 }
        &&& payload_window(&segment.header, bytes, window.0, window.1, segment.payload@)
    }
}

pub(super) proof fn decoder_fields(decoder: &crate::document::Decoder)
    ensures decoder_limits(decoder) == decoder.limits,
{}

pub(super) proof fn segment_metadata_append<'a>(
    segments: Seq<crate::document::ParsedSegment<'a>>,
    segment: crate::document::ParsedSegment<'a>,
)
    requires
        segment_metadata(segments),
        known_segment_data_fields(segment.header.segment_type, segment.payload@, segment.data),
    ensures segment_metadata(segments.push(segment)),
{
    let appended = segments.push(segment);
    assert forall|index: int| 0 <= index < appended.len() implies
        #[trigger] known_segment_data_fields(
            appended[index].header.segment_type, appended[index].payload@, appended[index].data,
        ) by {
        if index < segments.len() {
            assert(appended[index] == segments[index]);
        } else {
            assert(index == segments.len());
            assert(appended[index] == segment);
        }
    }
}

pub(super) proof fn document_fields(document: &crate::document::DocumentInfo)
    ensures
        document_organization(document) == document.organization,
        document_declared_page_count(document) == document.declared_page_count,
        document_segment_count(document) == document.segment_count,
        document_pages(document) == document.pages@,
{}

pub(super) proof fn global_fields(globals: &crate::document::GlobalSegments)
    ensures
        global_segment_count(globals) == globals.types@.len(),
        global_depth_count(globals) == globals.depths@.len(),
        global_types(globals) == globals.types@,
        global_bytes(globals) == globals.bytes@,
{}

pub(super) proof fn sequential_window_append<'a>(
    bytes: Seq<u8>,
    segments: Seq<crate::document::ParsedSegment<'a>>,
    windows: Seq<(int, int, int)>,
    end: int,
    segment: crate::document::ParsedSegment<'a>,
    payload_start: int,
    next_end: int,
    ordered: bool,
)
    requires
        sequential_windows(bytes, segments, windows, end, ordered),
        end < payload_start <= next_end <= bytes.len(),
        crate::spec::segments::segment_header_fields(
            &segment.header, bytes, end, payload_start,
        ),
        payload_window(&segment.header, bytes, payload_start, next_end, segment.payload@),
        ordered && segments.len() > 0 ==>
            segments.last().header.number < segment.header.number,
    ensures sequential_windows(
        bytes, segments.push(segment), windows.push((end, payload_start, next_end)),
        next_end, ordered,
    ),
{
    let extended = windows.push((end, payload_start, next_end));
    let appended = segments.push(segment);
    assert forall|index: int| 0 <= index < appended.len() implies {
        let window = #[trigger] extended[index];
        let current = appended[index];
        &&& 0 <= window.0 < window.1 <= window.2 <= next_end
        &&& window.0 == if index == 0 { 0 } else { extended[index - 1].2 }
        &&& crate::spec::segments::segment_header_fields(
            &current.header, bytes, window.0, window.1,
        )
        &&& payload_window(&current.header, bytes, window.1, window.2, current.payload@)
        &&& (ordered && index > 0 ==> appended[index - 1].header.number < current.header.number)
    } by {
        if index < segments.len() {
            assert(extended[index] == windows[index]);
            assert(appended[index] == segments[index]);
        } else {
            assert(index == segments.len());
            if index > 0 {
                assert(extended[index - 1].2 == end);
            }
        }
    }
}

pub(super) proof fn segment_header_window_length(
    header: &crate::segment_header::SegmentHeader,
    bytes: Seq<u8>,
    start: int,
    end: int,
)
    requires crate::spec::segments::segment_header_fields(header, bytes, start, end),
    ensures header.byte_length == end - start,
{
    reveal(crate::spec::segments::segment_header_fields);
}

pub(super) proof fn sequential_offset_window(
    bytes: Seq<u8>,
    segments: Seq<crate::document::ParsedSegment<'_>>,
    windows: Seq<(int, int, int)>,
    end: int,
    ordered: bool,
    prefix: nat,
)
    requires sequential_windows(bytes, segments, windows, end, ordered),
        prefix <= segments.len(),
    ensures sequential_offset(segments, prefix) == if prefix == 0 {
        0int
    } else {
        windows[(prefix - 1) as int].2
    },
    decreases prefix,
{
    hide(crate::spec::segments::segment_header_fields);
    reveal_with_fuel(sequential_offset, 1);
    if prefix > 0 {
        sequential_offset_window(bytes, segments, windows, end, ordered, (prefix - 1) as nat);
        let segment = segments[(prefix - 1) as int];
        let window = windows[(prefix - 1) as int];
        segment_header_window_length(&segment.header, bytes, window.0, window.1);
        assert(segment.payload@.len() == window.2 - window.1);
    }
}

pub(super) proof fn sequential_segment_from_windows(
    bytes: Seq<u8>,
    segments: Seq<crate::document::ParsedSegment<'_>>,
    windows: Seq<(int, int, int)>,
    ordered: bool,
    index: int,
)
    requires
        sequential_windows(bytes, segments, windows, bytes.len() as int, ordered),
        segment_metadata(segments),
        0 <= index < segments.len(),
    ensures {
        let segment = segments[index];
        let start = sequential_offset(segments, index as nat) as int;
        let payload_start = start + segment.header.byte_length;
        let end = sequential_offset(segments, (index + 1) as nat) as int;
        &&& 0 <= start < payload_start <= end <= bytes.len()
        &&& crate::spec::segments::segment_header_fields(
            &segment.header, bytes, start, payload_start,
        )
        &&& payload_window(&segment.header, bytes, payload_start, end, segment.payload@)
        &&& (ordered && index > 0 ==> segments[index - 1].header.number < segment.header.number)
    },
{
    hide(crate::spec::segments::segment_header_fields);
    hide(payload_window);
    hide(sequential_windows);
    hide(sequential_offset);
    sequential_offset_window(bytes, segments, windows, bytes.len() as int, ordered, index as nat);
    sequential_offset_window(bytes, segments, windows, bytes.len() as int, ordered, (index + 1) as nat);
    assert(windows.len() == segments.len()) by {
        reveal(sequential_windows);
    }
    let window = windows[index];
    assert({
        &&& 0 <= window.0 < window.1 <= window.2 <= bytes.len()
        &&& window.0 == if index == 0 { 0 } else { windows[index - 1].2 }
        &&& crate::spec::segments::segment_header_fields(
            &segments[index].header, bytes, window.0, window.1,
        )
        &&& payload_window(&segments[index].header, bytes, window.1, window.2, segments[index].payload@)
        &&& (ordered && index > 0 ==> segments[index - 1].header.number < segments[index].header.number)
    }) by {
        reveal(sequential_windows);
    }
    segment_header_window_length(&segments[index].header, bytes, window.0, window.1);
    assert(sequential_offset(segments, index as nat) == window.0);
    assert(sequential_offset(segments, (index + 1) as nat) == window.2);
}

pub(super) proof fn sequential_complete_fields(
    bytes: Seq<u8>,
    segments: Seq<crate::document::ParsedSegment<'_>>,
    windows: Seq<(int, int, int)>,
    ordered: bool,
)
    requires sequential_windows(bytes, segments, windows, bytes.len() as int, ordered),
        segment_metadata(segments),
    ensures sequential_container_fields(bytes, segments, ordered),
{
    sequential_offset_window(bytes, segments, windows, bytes.len() as int, ordered, segments.len());
    reveal(sequential_container_fields);
    assert forall|index: int| 0 <= index < segments.len() implies {
        let segment = #[trigger] segments[index];
        let start = sequential_offset(segments, index as nat) as int;
        let payload_start = start + segment.header.byte_length;
        let end = sequential_offset(segments, (index + 1) as nat) as int;
        &&& 0 <= start < payload_start <= end <= bytes.len()
        &&& crate::spec::segments::segment_header_fields(
            &segment.header, bytes, start, payload_start,
        )
        &&& payload_window(&segment.header, bytes, payload_start, end, segment.payload@)
        &&& (ordered && index > 0 ==> segments[index - 1].header.number < segment.header.number)
    } by {
        sequential_segment_from_windows(bytes, segments, windows, ordered, index);
    }
}

pub(super) proof fn random_header_window_append(
    bytes: Seq<u8>,
    headers: Seq<crate::segment_header::SegmentHeader>,
    windows: Seq<(int, int)>,
    end: int,
    header: crate::segment_header::SegmentHeader,
    next_end: int,
)
    requires
        random_header_windows(bytes, headers, windows, end),
        end < next_end <= bytes.len(),
        crate::spec::segments::segment_header_fields(&header, bytes, end, next_end),
        headers.len() > 0 ==> headers.last().number < header.number,
        headers.len() > 0 ==>
            headers.last().segment_type != crate::segment_header::SegmentType::EndOfFile,
    ensures random_header_windows(
        bytes, headers.push(header), windows.push((end, next_end)), next_end,
    ),
{
    let extended = windows.push((end, next_end));
    let appended = headers.push(header);
    assert forall|index: int| 0 <= index < appended.len() implies {
        let window = #[trigger] extended[index];
        &&& 0 <= window.0 < window.1 <= next_end
        &&& window.0 == if index == 0 { 0 } else { extended[index - 1].1 }
        &&& crate::spec::segments::segment_header_fields(
            &appended[index], bytes, window.0, window.1,
        )
        &&& (index > 0 ==> appended[index - 1].number < appended[index].number)
        &&& (index + 1 < appended.len() ==>
            appended[index].segment_type != crate::segment_header::SegmentType::EndOfFile)
    } by {
        if index < headers.len() {
            assert(extended[index] == windows[index]);
            assert(appended[index] == headers[index]);
            assert(crate::spec::segments::segment_header_fields(
                &headers[index], bytes, windows[index].0, windows[index].1,
            ));
            if index > 0 {
                assert(extended[index - 1] == windows[index - 1]);
                assert(appended[index - 1] == headers[index - 1]);
            }
            if index + 1 == headers.len() {
                assert(headers[index] == headers.last());
            }
        } else {
            assert(index == headers.len());
            assert(appended[index] == header);
            assert(extended[index] == (end, next_end));
            if index > 0 {
                assert(extended[index - 1].1 == end);
                assert(appended[index - 1] == headers.last());
            }
        }
    }
}

pub(super) proof fn random_payload_window_append<'a>(
    bytes: Seq<u8>,
    headers: Seq<crate::segment_header::SegmentHeader>,
    header_end: int,
    segments: Seq<crate::document::ParsedSegment<'a>>,
    windows: Seq<(int, int)>,
    end: int,
    segment: crate::document::ParsedSegment<'a>,
    next_end: int,
)
    requires
        random_payload_windows(bytes, headers, header_end, segments, windows, end),
        segments.len() < headers.len(),
        segment.header == headers[segments.len() as int],
        payload_window(&segment.header, bytes, end, next_end, segment.payload@),
    ensures random_payload_windows(
        bytes, headers, header_end, segments.push(segment),
        windows.push((end, next_end)), next_end,
    ),
{
    let appended = segments.push(segment);
    let extended = windows.push((end, next_end));
    assert forall|index: int| 0 <= index < appended.len() implies {
        let current = #[trigger] appended[index];
        let window = extended[index];
        &&& current.header == headers[index]
        &&& header_end <= window.0 <= window.1 <= next_end
        &&& window.0 == if index == 0 { header_end } else { extended[index - 1].1 }
        &&& payload_window(&current.header, bytes, window.0, window.1, current.payload@)
    } by {
        if index < segments.len() {
            assert(appended[index] == segments[index]);
            assert(extended[index] == windows[index]);
        } else if index > 0 {
            assert(extended[index - 1].1 == end);
        }
    }
}

pub(super) proof fn random_header_offset_window(
    bytes: Seq<u8>,
    headers: Seq<crate::segment_header::SegmentHeader>,
    windows: Seq<(int, int)>,
    end: int,
    prefix: nat,
)
    requires random_header_windows(bytes, headers, windows, end),
        prefix <= headers.len(),
    ensures header_byte_offset(headers, prefix) == if prefix == 0 {
        0int
    } else {
        windows[(prefix - 1) as int].1
    },
    decreases prefix,
{
    reveal_with_fuel(header_byte_offset, 1);
    if prefix > 0 {
        random_header_offset_window(bytes, headers, windows, end, (prefix - 1) as nat);
        let window = windows[(prefix - 1) as int];
        reveal(crate::spec::segments::segment_header_fields);
        assert(headers[(prefix - 1) as int].byte_length == window.1 - window.0);
    }
}

pub(super) proof fn random_payload_offset_window(
    bytes: Seq<u8>,
    headers: Seq<crate::segment_header::SegmentHeader>,
    header_end: int,
    segments: Seq<crate::document::ParsedSegment<'_>>,
    windows: Seq<(int, int)>,
    end: int,
    prefix: nat,
)
    requires random_payload_windows(bytes, headers, header_end, segments, windows, end),
        prefix <= segments.len(),
    ensures payload_byte_offset(segments, prefix) == if prefix == 0 {
        0int
    } else {
        windows[(prefix - 1) as int].1 - header_end
    },
    decreases prefix,
{
    reveal_with_fuel(payload_byte_offset, 1);
    if prefix > 0 {
        random_payload_offset_window(
            bytes, headers, header_end, segments, windows, end, (prefix - 1) as nat,
        );
        let window = windows[(prefix - 1) as int];
        assert(segments[(prefix - 1) as int].payload@.len() == window.1 - window.0);
    }
}

pub(super) proof fn random_access_segment_from_windows(
    bytes: Seq<u8>,
    headers: Seq<crate::segment_header::SegmentHeader>,
    header_windows: Seq<(int, int)>,
    header_end: int,
    segments: Seq<crate::document::ParsedSegment<'_>>,
    payload_windows: Seq<(int, int)>,
    index: int,
)
    requires
        random_header_windows(bytes, headers, header_windows, header_end),
        random_payload_windows(
            bytes, headers, header_end, segments, payload_windows, bytes.len() as int,
        ),
        segments.len() == headers.len(),
        segment_headers(segments) == headers,
        0 <= index < segments.len(),
    ensures random_access_segment_fields(bytes, segments, index),
{
    random_header_offset_window(bytes, headers, header_windows, header_end, headers.len());
    random_header_offset_window(bytes, headers, header_windows, header_end, index as nat);
    random_header_offset_window(bytes, headers, header_windows, header_end, (index + 1) as nat);
    random_payload_offset_window(
        bytes, headers, header_end, segments, payload_windows, bytes.len() as int, index as nat,
    );
    random_payload_offset_window(
        bytes, headers, header_end, segments, payload_windows, bytes.len() as int, (index + 1) as nat,
    );
    assert(segments[index].header == headers[index]);
    assert(crate::spec::segments::segment_header_fields(
        &headers[index], bytes, header_windows[index].0, header_windows[index].1,
    ));
    assert(header_windows[index].0 == header_byte_offset(headers, index as nat));
    assert(payload_windows[index].0 == header_end + payload_byte_offset(segments, index as nat));
    reveal(random_access_segment_fields);
}

pub(super) proof fn random_access_complete_fields(
    bytes: Seq<u8>,
    headers: Seq<crate::segment_header::SegmentHeader>,
    header_windows: Seq<(int, int)>,
    header_end: int,
    segments: Seq<crate::document::ParsedSegment<'_>>,
    payload_windows: Seq<(int, int)>,
)
    requires
        random_header_windows(bytes, headers, header_windows, header_end),
        headers.len() > 0,
        headers.last().segment_type == crate::segment_header::SegmentType::EndOfFile,
        random_payload_windows(
            bytes, headers, header_end, segments, payload_windows, bytes.len() as int,
        ),
        segments.len() == headers.len(),
        segment_metadata(segments),
    ensures random_access_container_fields(bytes, segments),
{
    assert forall|index: int| 0 <= index < segments.len() implies
        #[trigger] segment_headers(segments)[index] == headers[index] by {
        assert(segments[index].header == headers[index]);
    }
    vstd::assert_seqs_equal!(segment_headers(segments), headers);
    random_header_offset_window(bytes, headers, header_windows, header_end, headers.len());
    random_payload_offset_window(
        bytes, headers, header_end, segments, payload_windows, bytes.len() as int, segments.len(),
    );
    reveal(random_access_container_fields);
    assert forall|index: int| 0 <= index < segments.len() implies
        #[trigger] random_access_segment_fields(bytes, segments, index) by {
        random_access_segment_from_windows(
            bytes, headers, header_windows, header_end, segments, payload_windows, index,
        );
    }
}

}
