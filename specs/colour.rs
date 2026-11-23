//! RGB storage and presentation policy, not full colour conformance.
use vstd::prelude::*;
verus! {
// Model navigation: spec::width [proof-support]
// Purpose: Opaque public RGB width projection.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn width(bitmap: &super::RgbBitmap) -> u32 { bitmap.width }
// Model navigation: spec::height [proof-support]
// Purpose: Opaque public RGB height projection.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn height(bitmap: &super::RgbBitmap) -> u32 { bitmap.height }
// Model navigation: spec::stride [proof-support]
// Purpose: Opaque public RGB row-size projection.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn stride(bitmap: &super::RgbBitmap) -> usize { bitmap.stride }
// Model navigation: spec::bytes [proof-support]
// Purpose: Opaque public RGB byte projection.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn bytes(bitmap: &super::RgbBitmap) -> Seq<u8> { bitmap.bytes@ }

pub(crate) proof fn bitmap_fields(bitmap: &super::RgbBitmap)
    ensures width(bitmap) == bitmap.width, height(bitmap) == bitmap.height,
        stride(bitmap) == bitmap.stride, bytes(bitmap) == bitmap.bytes@,
{
    reveal(width); reveal(height); reveal(stride); reveal(bytes);
}

pub(crate) proof fn pixel_offset_bounds(bitmap: &super::RgbBitmap, x: u32, y: u32)
    requires storage_valid(bitmap), x < bitmap.width, y < bitmap.height,
    ensures
        x as int * 3 + 2 < bitmap.stride,
        y as int * bitmap.stride + x as int * 3 + 2 < bitmap.bytes@.len(),
        x as int * 3 <= usize::MAX,
        y as int * bitmap.stride <= usize::MAX,
        y as int * bitmap.stride + x as int * 3 + 2 <= usize::MAX,
{
    bitmap_fields(bitmap);
    assert(x as int >= 0 && x < bitmap.width && bitmap.stride as int == bitmap.width as int * 3
        ==> x as int * 3 + 2 < bitmap.stride) by(nonlinear_arith);
    assert(x as int >= 0 && y as int >= 0 && x < bitmap.width && y < bitmap.height
        && bitmap.stride as int == bitmap.width as int * 3
        && bitmap.bytes@.len() == bitmap.stride as int * bitmap.height as int
        ==> y as int * bitmap.stride + x as int * 3 + 2 < bitmap.bytes@.len()) by(nonlinear_arith);
}

// Model navigation: spec::page_number [proof-support]
// Purpose: Opaque public colour page-number projection.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn page_number(page: &super::ColourPage) -> u32 { page.number }
// Model navigation: spec::page_bitmap [proof-support]
// Purpose: Opaque public colour page-bitmap projection.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn page_bitmap(page: &super::ColourPage) -> &super::RgbBitmap { &page.bitmap }
// Model navigation: spec::pages [proof-support]
// Purpose: Opaque public complete-page projection.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub closed spec fn pages(document: &super::DecodedColourDocument) -> Seq<super::ColourPage> { document.pages@ }
pub(crate) proof fn page_fields(page: &super::ColourPage)
    ensures page_number(page) == page.number, page_bitmap(page) == &page.bitmap,
{ reveal(page_number); reveal(page_bitmap); }
pub(crate) proof fn document_fields(document: &super::DecodedColourDocument)
    ensures pages(document) == document.pages@,
{ reveal(pages); }
// Model navigation: spec::storage_valid [representation-invariant]
// Purpose: Exact three-channel RGB row and buffer extents.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn storage_valid(bitmap: &super::RgbBitmap) -> bool {
    stride(bitmap) as nat == width(bitmap) as nat * 3
        && bytes(bitmap).len() == stride(bitmap) as nat * height(bitmap) as nat
        && stride(bitmap) as nat * height(bitmap) as nat <= usize::MAX
}

// Model navigation: spec::pixel [project-result-model]
// Purpose: Exact bounds-checked RGB channel projection.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn pixel(bitmap: &super::RgbBitmap, x: u32, y: u32) -> Option<[u8;3]> {
    if x >= width(bitmap) || y >= height(bitmap) { None } else {
        let offset = y as int * stride(bitmap) as int + x as int * 3;
        Some([bytes(bitmap)[offset], bytes(bitmap)[offset+1], bytes(bitmap)[offset+2]])
    }
}

// Model navigation: spec::paint_channel [project-policy]
// Purpose: RGB foreground transparency and monochrome OR/REPLACE presentation.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(crate) open spec fn paint_channel(
    old: u8, ink: bool, foreground: Option<[u8;3]>, replace: bool, channel: int,
) -> u8 {
    match foreground {
        Some(rgb) => if ink {rgb@[channel]} else {old},
        None => if ink {0u8} else if replace {255u8} else {old},
    }
}

// Model navigation: spec::palette_contents [project-result-model]
// Purpose: Exact RGB8 triples and count from the encoded palette.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub(super) open spec fn palette_contents(bytes: Seq<u8>, colours: Seq<[u8;3]>) -> bool {
    bytes.len() == 7 + colours.len() * 3
        && colours.len() == crate::spec::segments::be_u32_at(bytes, 3)
        && (forall|index:int, channel:int| 0 <= index < colours.len() && 0 <= channel < 3 ==>
            colours[index]@[channel] == bytes[7 + index * 3 + channel])
}

// Model navigation: spec::document_valid [representation-invariant]
// Purpose: Every returned page is complete, positive and backed by valid RGB storage.
// Mapping: no registered requirement/locator mapping.
// Fidelity: unresolved; no independent normative approval.
pub open spec fn document_valid(document: &super::DecodedColourDocument) -> bool {
    forall|index:int| 0 <= index < pages(document).len() ==>
        page_number(&pages(document)[index]) > 0 && storage_valid(page_bitmap(&pages(document)[index]))
}
}
