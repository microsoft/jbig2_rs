#![forbid(unsafe_code)]
//! Untimed ordinary embedded API output. Raw frames go only to a hashing pipe.
use std::fs;
use std::io::{self, Write};

use jbig2_decoder::{
    DecodedDocument, Decoder, EmbeddedPolicy, EmbeddedRecovery, EmbeddedRecoveryKind, Organization,
};

fn number(out: &mut impl Write, value: u64) -> io::Result<()> {
    out.write_all(&value.to_be_bytes())
}

fn text(out: &mut impl Write, value: &str) -> io::Result<()> {
    number(out, value.len() as u64)?;
    out.write_all(value.as_bytes())
}

fn document(out: &mut impl Write, value: &DecodedDocument) -> io::Result<()> {
    let info = value.info();
    out.write_all(&[match info.organization() {
        Organization::Sequential => 0,
        Organization::RandomAccess => 1,
    }])?;
    match info.declared_page_count() {
        None => out.write_all(&[0])?,
        Some(count) => {
            out.write_all(&[1])?;
            number(out, u64::from(count))?;
        }
    }
    number(out, u64::from(info.segment_count()))?;
    number(out, info.pages().len() as u64)?;
    for page in info.pages() {
        for field in [
            page.number(),
            page.width(),
            page.height(),
            page.x_resolution(),
            page.y_resolution(),
        ] {
            number(out, u64::from(field))?;
        }
        out.write_all(&[u8::from(page.is_striped())])?;
    }
    number(out, value.pages().len() as u64)?;
    for page in value.pages() {
        number(out, u64::from(page.number()))?;
        let bitmap = page.bitmap();
        number(out, u64::from(bitmap.width()))?;
        number(out, u64::from(bitmap.height()))?;
        number(out, bitmap.stride() as u64)?;
        number(out, bitmap.bytes().len() as u64)?;
        out.write_all(bitmap.bytes())?;
    }
    Ok(())
}

fn recoveries(out: &mut impl Write, values: &[EmbeddedRecovery]) -> io::Result<()> {
    number(out, values.len() as u64)?;
    for value in values {
        number(out, u64::from(value.segment))?;
        number(out, u64::from(value.page))?;
        number(out, value.byte_offset as u64)?;
        out.write_all(&[match value.kind {
            EmbeddedRecoveryKind::AuxiliaryBuffersFlag => 0,
            EmbeddedRecoveryKind::RefinementsFlag => 1,
            EmbeddedRecoveryKind::ArithmeticTerminatorCode => 2,
        }])?;
    }
    Ok(())
}

fn run(out: &mut impl Write, manifest: &str) -> Result<(), Box<dyn std::error::Error>> {
    let input = fs::read_to_string(manifest)?;
    let rows: Vec<_> = input.lines().collect();
    if rows.len() != 321 {
        return Err("all321 normalized requests are mandatory".into());
    }
    out.write_all(b"J321API1")?;
    let mut frames = 0;
    for row in rows {
        let fields: Vec<_> = row.split('\t').collect();
        if fields.len() != 3 {
            return Err("invalid request TSV".into());
        }
        let payload = fs::read(fields[1])?;
        let global_bytes = if fields[2] == "-" {
            None
        } else {
            Some(fs::read(fields[2])?)
        };
        for (code, policy) in [(0, EmbeddedPolicy::Strict), (1, EmbeddedPolicy::BestEffort)] {
            eprintln!("request={} policy={code}", fields[0]);
            let decoder = Decoder::default();
            out.write_all(b"CASE")?;
            text(out, fields[0])?;
            out.write_all(&[code])?;
            let globals = match &global_bytes {
                None => {
                    out.write_all(&[0])?;
                    None
                }
                Some(bytes) => {
                    out.write_all(&[1])?;
                    match decoder.parse_globals(bytes) {
                        Ok(value) => {
                            out.write_all(&[1])?;
                            number(out, value.segment_count() as u64)?;
                            Some(value)
                        }
                        Err(error) => {
                            out.write_all(&[0])?;
                            text(out, &format!("{error:?}"))?;
                            out.write_all(b"DONE")?;
                            frames += 1;
                            continue;
                        }
                    }
                }
            };
            match decoder.decode_embedded_with_policy(&payload, globals.as_ref(), policy) {
                Ok(result) => {
                    out.write_all(&[1])?;
                    document(out, &result.value)?;
                    recoveries(out, &result.recoveries)?;
                }
                Err(error) => {
                    out.write_all(&[0])?;
                    // This exact derived-Debug error type closure is source-audited
                    // and hash-pinned by the driver. Pixels never use Debug.
                    text(out, &format!("{error:?}"))?;
                }
            }
            out.write_all(b"DONE")?;
            frames += 1;
        }
    }
    out.write_all(b"END!")?;
    number(out, frames)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err("usage: ordinary-cohort-serialize HASH_CHECKED_REQUESTS_TSV".into());
    }
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    run(&mut out, &args[0])?;
    out.flush()?;
    Ok(())
}
