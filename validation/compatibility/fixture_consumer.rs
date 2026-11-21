use std::{env, fs, io::Cursor, io::Write, path::Path};

use jbig2_decoder::{DecodeError, DecodePolicy, DecodeResult, DecodedDocument, Decoder, Limits};

fn assert_replay(
    first: &Result<DecodeResult<DecodedDocument>, DecodeError>,
    repeated: &Result<DecodeResult<DecodedDocument>, DecodeError>,
) {
    match (first, repeated) {
        (Ok(first), Ok(repeated)) => {
            assert_eq!(first.value, repeated.value);
            assert_eq!(first.recoveries, repeated.recoveries);
        }
        (Err(first), Err(repeated)) => assert_eq!(format!("{first:?}"), format!("{repeated:?}")),
        (first, repeated) => panic!("context replay changed outcome: {first:?}/{repeated:?}"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: consumer INPUT OUTPUT MODE MAX_WORK".into());
    }
    let input = fs::read(&args[0])?;
    let mut limits = Limits::default();
    if args[3] != "default" {
        limits.max_work_units = args[3].parse()?;
    }
    let policy = if args[2].starts_with("compat") {
        DecodePolicy::Compatibility
    } else {
        DecodePolicy::Strict
    };
    let decoder = Decoder::new(limits);
    let mut context = decoder.context();
    let result = match args[2].as_str() {
        "strict" | "compat" => decoder.decode_with_policy(&input, policy),
        "strict-context" | "compat-context" => {
            let first = context.decode_with_policy(&input, policy);
            let repeated = context.decode_with_policy(&input, policy);
            assert_replay(&first, &repeated);
            first
        }
        "strict-reader" | "compat-reader" => {
            decoder.decode_reader_with_policy(&mut Cursor::new(&input), policy)
        }
        "strict-context-reader" | "compat-context-reader" => {
            let first = context.decode_reader_with_policy(&mut Cursor::new(&input), policy);
            let repeated = context.decode_reader_with_policy(&mut Cursor::new(&input), policy);
            assert_replay(&first, &repeated);
            first
        }
        _ => return Err("unknown consumer mode".into()),
    };
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            println!("ERROR|{error:?}");
            std::process::exit(2);
        }
    };
    println!("OK|{}", result.value.pages().len());
    println!("RECOVERIES|{:?}", result.recoveries);
    fs::create_dir_all(&args[1])?;
    for page in result.value.pages() {
        let bitmap = page.bitmap();
        let mut file =
            fs::File::create(Path::new(&args[1]).join(format!("page-{}.raw", page.number())))?;
        for value in [
            page.number(),
            bitmap.width(),
            bitmap.height(),
            u32::try_from(bitmap.stride())?,
        ] {
            file.write_all(&value.to_be_bytes())?;
        }
        file.write_all(bitmap.bytes())?;
    }
    Ok(())
}
