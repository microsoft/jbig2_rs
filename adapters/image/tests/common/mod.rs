pub fn segment(number: u32, kind: u8, page: u8, payload: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let mut header = number.to_be_bytes().to_vec();
    header.extend_from_slice(&[kind, 0, page]);
    header.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    (header, payload)
}

pub fn page(
    number: u32,
    association: u8,
    width: u32,
    height: u32,
    striped: bool,
) -> (Vec<u8>, Vec<u8>) {
    let mut payload = width.to_be_bytes().to_vec();
    payload.extend_from_slice(&height.to_be_bytes());
    payload.extend_from_slice(&[0; 8]);
    payload.push(0x44);
    payload.extend_from_slice(&(if striped { 0x8002_u16 } else { 0 }).to_be_bytes());
    segment(number, 48, association, payload)
}

pub fn white_region(number: u32, width: u32, x: u32, y: u32) -> (Vec<u8>, Vec<u8>) {
    let mut payload = width.to_be_bytes().to_vec();
    payload.extend_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(&x.to_be_bytes());
    payload.extend_from_slice(&y.to_be_bytes());
    payload.extend_from_slice(&[4, 1, 0x80]);
    segment(number, 38, 1, payload)
}

pub fn standalone(
    sequential: bool,
    pages: Option<u32>,
    segments: Vec<(Vec<u8>, Vec<u8>)>,
) -> Vec<u8> {
    let mut bytes = b"\x97JB2\r\n\x1a\n".to_vec();
    bytes.push(u8::from(sequential) | (u8::from(pages.is_none()) << 1));
    if let Some(pages) = pages {
        bytes.extend_from_slice(&pages.to_be_bytes());
    }
    if sequential {
        for (header, payload) in segments {
            bytes.extend(header);
            bytes.extend(payload);
        }
    } else {
        for (header, _) in &segments {
            bytes.extend_from_slice(header);
        }
        for (_, payload) in segments {
            bytes.extend(payload);
        }
    }
    bytes
}

pub fn patterned(sequential: bool, striped: bool) -> Vec<u8> {
    let mut segments = vec![
        page(0, 1, 9, if striped { u32::MAX } else { 3 }, striped),
        white_region(1, 3, 2, 0),
        white_region(2, 9, 0, 1),
    ];
    if striped {
        segments.push(segment(3, 50, 1, 2_u32.to_be_bytes().to_vec()));
    }
    segments.push(segment(4, 49, 1, vec![]));
    segments.push(segment(5, 51, 0, vec![]));
    standalone(sequential, Some(1), segments)
}

pub fn expected() -> Vec<u8> {
    [&[0, 0, 255, 255, 255, 0, 0, 0, 0][..], &[255; 9], &[0; 9]].concat()
}
