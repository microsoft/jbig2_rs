//! Ordinary public RGB API replay consumer, not a proof entry point.
use jbig2_decoder::{DecodePolicy, Decoder};
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 { return Err("usage: INPUT OUTPUT strict|strict-policy|compat".into()); }
    let input = fs::read(&args[0])?;
    let path = PathBuf::from(&args[1]);
    let decoder = Decoder::default();
    let result = match args[2].as_str() {
        "strict" => decoder.decode_rgb(&input).map(|value| jbig2_decoder::DecodeResult {value, recoveries:Vec::new()}),
        "strict-policy" => decoder.decode_rgb_with_policy(&input, DecodePolicy::Strict),
        "compat" => decoder.decode_rgb_with_policy(&input, DecodePolicy::Compatibility),
        _ => return Err("unknown RGB policy".into()),
    };
    match result {
        Ok(output) => {
            fs::create_dir_all(&path)?;
            println!("OK|{}", output.value.pages().len());
            println!("RECOVERIES|{:?}", output.recoveries);
            for page in output.value.pages() {
                let bitmap = page.bitmap();
                let mut bytes = Vec::new();
                for field in [page.number(), bitmap.width(), bitmap.height(), u32::try_from(bitmap.stride())?] {
                    bytes.extend_from_slice(&field.to_be_bytes());
                }
                bytes.extend_from_slice(bitmap.bytes());
                fs::write(path.join(format!("page-{}.rgb", page.number())), bytes)?;
            }
        }
        Err(error) => {
            println!("ERROR|{error:?}");
            std::process::exit(2);
        }
    }
    Ok(())
}
