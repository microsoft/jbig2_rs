//! Built and executed only at the fixed BASE, never at the candidate.
use jbig2_decoder::{Decoder, EmbeddedPolicy};
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Deserialize)]
struct Case {
    id: String,
    input: String,
    globals: Option<String>,
    width: u32,
    height: u32,
    pages: usize,
    policy: String,
    standalone: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: BASE-materializer MANIFEST OUTPUT_DIRECTORY".into());
    }
    let cases: Vec<Case> = serde_json::from_slice(&fs::read(&args[0])?)?;
    let directory = Path::new(&args[1]);
    fs::create_dir_all(directory)?;
    let mut result = Vec::new();
    for case in cases {
        eprintln!("BASE golden case: {}", case.id);
        let decoder = Decoder::default();
        let input = fs::read(&case.input)?;
        let globals = case
            .globals
            .as_ref()
            .map(fs::read)
            .transpose()?
            .map(|bytes| decoder.parse_globals(&bytes))
            .transpose()?;
        let document = if case.standalone {
            decoder.decode(&input)?
        } else {
            decoder
                .decode_embedded_with_policy(
                    &input,
                    globals.as_ref(),
                    if case.policy == "strict" {
                        EmbeddedPolicy::Strict
                    } else {
                        EmbeddedPolicy::BestEffort
                    },
                )?
                .value
        };
        if document.pages().len() != case.pages || case.pages == 0 {
            return Err(format!("wrong BASE page count: {}", case.id).into());
        }
        let expected = document.pages()[0].bitmap().bytes();
        for (index, page) in document.pages().iter().enumerate() {
            if page.number() as usize != index + 1
                || page.bitmap().width() != case.width
                || page.bitmap().height() != case.height
                || page.bitmap().bytes() != expected
            {
                return Err(
                    format!("wrong BASE shape or unequal per-page output: {}", case.id).into(),
                );
            }
        }
        let path = directory.join(format!("{}.packed", case.id));
        if path.exists() {
            return Err("refusing to overwrite an existing golden".into());
        }
        fs::write(path, expected)?;
        result.push(serde_json::json!({
            "id":case.id,"width":case.width,"height":case.height,"pages":case.pages,
            "bytes_per_page":expected.len(),
            "scope":"BASE output; Python must independently verify every frozen SHA before use",
        }));
    }
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
