use jbig2_decoder::{
    DecodedDocument, Decoder, DocumentInfo, EmbeddedPolicy, EmbeddedRecovery, GlobalSegments,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
struct Case {
    id: String,
    family: String,
    input: String,
    globals: Option<String>,
    expected: String,
    width: u32,
    height: u32,
    pages: usize,
    policy: String,
    standalone: bool,
}

enum Output {
    Document(DecodedDocument),
    Recovered(DecodedDocument, Vec<EmbeddedRecovery>),
    Info(DocumentInfo),
    Globals(GlobalSegments),
}

fn validate(
    output: &Output,
    case: &Case,
    expected: &[u8],
    global_count: usize,
) -> Result<(), String> {
    let document = match output {
        Output::Document(document) | Output::Recovered(document, _) => Some(document),
        _ => None,
    };
    if let Some(document) = document {
        if document.pages().len() != case.pages {
            return Err("decoded page count mismatch".into());
        }
        for (index, page) in document.pages().iter().enumerate() {
            let bitmap = page.bitmap();
            if page.number() as usize != index + 1
                || (bitmap.width(), bitmap.height()) != (case.width, case.height)
                || bitmap.bytes() != expected
            {
                return Err("exact decoded bitmap mismatch".into());
            }
        }
    }
    if let Output::Info(info) = output {
        if info.pages().len() != case.pages
            || info
                .pages()
                .iter()
                .any(|page| (page.width(), page.height()) != (case.width, case.height))
        {
            return Err("inspected metadata mismatch".into());
        }
    }
    if let Output::Globals(globals) = output {
        if globals.segment_count() != global_count {
            return Err("parsed globals count mismatch".into());
        }
    }
    Ok(())
}
