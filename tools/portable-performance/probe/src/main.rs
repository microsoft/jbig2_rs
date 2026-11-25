use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use jbig2_decoder::{
    DecodedDocument, Decoder, DocumentInfo, EmbeddedPolicy, EmbeddedRecovery, GlobalSegments,
};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[path = "../../../../benchmarks/cases.rs"]
mod generated_cases;

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

#[inline(never)]
fn profile_run(
    operation: &mut impl FnMut() -> Result<Output, String>,
    repetitions: usize,
) -> Result<Output, String> {
    let mut last = None;
    for _ in 0..repetitions {
        last = Some(black_box(operation()?));
    }
    last.ok_or_else(|| "profile repetitions must be positive".into())
}

fn peak_rss() -> Result<u64, Box<dyn std::error::Error>> {
    let status = fs::read_to_string("/proc/self/status")?;
    let line = status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))
        .ok_or("missing VmHWM")?;
    let parts: Vec<_> = line.split_whitespace().collect();
    if parts.len() != 3 || parts[2] != "kB" {
        return Err("unexpected VmHWM format".into());
    }
    Ok(parts[1].parse()?)
}

fn generate(directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(directory)?;
    let mut result = Vec::new();
    for case in generated_cases::cases(None) {
        if case.outcome != "decoded" {
            continue;
        }
        let input = directory.join(format!("{}.jb2", case.id));
        let expected = directory.join(format!("{}.packed", case.id));
        fs::write(&input, &case.input)?;
        let packed = vec![case.pattern; case.width.div_ceil(8) as usize * case.height as usize];
        fs::write(&expected, packed)?;
        result.push(Case {
            id: format!("generated-{}", case.id),
            family: if case.pattern == 0 {
                "generated-mmr-white"
            } else {
                "generated-mmr-transitions"
            }
            .into(),
            input: input.to_string_lossy().into(),
            globals: None,
            expected: expected.to_string_lossy().into(),
            width: case.width,
            height: case.height,
            pages: case.pages as usize,
            policy: "strict".into(),
            standalone: true,
        });
    }
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "generate") && args.len() == 2 {
        return generate(Path::new(&args[1]));
    }
    if args.len() != 6 {
        return Err(
            "usage: probe measure|profile|alloc MANIFEST INDEX MODE SAMPLES TARGET_MS_OR_REPS"
                .into(),
        );
    }
    let cases: Vec<Case> = serde_json::from_slice(&fs::read(&args[1])?)?;
    let index: usize = args[2].parse()?;
    let case = cases.get(index).ok_or("case index out of bounds")?;
    let input = fs::read(&case.input)?;
    let expected = fs::read(&case.expected)?;
    if expected.len() != case.width.div_ceil(8) as usize * case.height as usize {
        return Err("invalid expected bitmap byte count".into());
    }
    let globals_bytes = case.globals.as_ref().map(fs::read).transpose()?;
    let decoder = Decoder::default();
    let globals = globals_bytes
        .as_ref()
        .map(|bytes| decoder.parse_globals(bytes))
        .transpose()?;
    let global_count = globals.as_ref().map_or(0, GlobalSegments::segment_count);
    let mut context = decoder.context();
    let mode = &args[3];
    let policy = if mode.starts_with("best-effort") {
        EmbeddedPolicy::BestEffort
    } else {
        EmbeddedPolicy::Strict
    };
    if case.policy != "strict" && !mode.starts_with("best-effort") {
        return Err("nonconforming fixture requires explicitly selected best effort".into());
    }
    let mut operation = || -> Result<Output, String> {
        let convert = |error: jbig2_decoder::DecodeError| format!("{error:?}");
        match mode.as_str() {
            "inspect" if case.standalone => {
                decoder.inspect(&input).map(Output::Info).map_err(convert)
            }
            "inspect" => match &globals {
                Some(globals) => decoder.inspect_embedded_with_globals(&input, globals),
                None => decoder.inspect_embedded(&input),
            }
            .map(Output::Info)
            .map_err(convert),
            "fresh" if case.standalone => decoder
                .decode(&input)
                .map(Output::Document)
                .map_err(convert),
            "fresh" => match &globals {
                Some(globals) => decoder.decode_embedded_with_globals(&input, globals),
                None => decoder.decode_embedded(&input),
            }
            .map(Output::Document)
            .map_err(convert),
            "context" if case.standalone => context
                .decode(&input)
                .map(Output::Document)
                .map_err(convert),
            "context" => match &globals {
                Some(globals) => context.decode_embedded_with_globals(&input, globals),
                None => context.decode_embedded(&input),
            }
            .map(Output::Document)
            .map_err(convert),
            "best-effort" | "policy-strict" => decoder
                .decode_embedded_with_policy(&input, globals.as_ref(), policy)
                .map(|result| Output::Recovered(result.value, result.recoveries))
                .map_err(convert),
            "best-effort-context" => context
                .decode_embedded_with_policy(&input, globals.as_ref(), policy)
                .map(|result| Output::Recovered(result.value, result.recoveries))
                .map_err(convert),
            "parse-globals" => decoder
                .parse_globals(globals_bytes.as_ref().ok_or("no globals fixture")?)
                .map(Output::Globals)
                .map_err(convert),
            "reparse-globals" => {
                let state = decoder
                    .parse_globals(globals_bytes.as_ref().ok_or("no globals fixture")?)
                    .map_err(convert)?;
                decoder
                    .decode_embedded_with_globals(&input, &state)
                    .map(Output::Document)
                    .map_err(convert)
            }
            "reader" if case.standalone => decoder
                .decode_reader(&mut input.as_slice())
                .map(Output::Document)
                .map_err(convert),
            _ => Err("unsupported mode for input envelope".into()),
        }
    };
    let first_start = Instant::now();
    let first = operation()?;
    let first_ns = first_start.elapsed().as_nanos();
    validate(&first, case, &expected, global_count)?;
    let recoveries = match &first {
        Output::Recovered(_, recoveries) => format!("{recoveries:?}"),
        _ => "[]".into(),
    };
    drop(first);
    if args[0] == "once" {
        println!(
            "{}",
            serde_json::json!({
                "id":case.id,"mode":mode,"policy":case.policy,"median_ns":first_ns,
                "samples_ns":[first_ns],"samples":1,"batch":1,"exact_output_checked":true,
                "width":case.width,"height":case.height,"pages":case.pages,
                "peak_rss_kib":peak_rss()?,"recoveries":recoveries,
                "scope":"single cold decode; input/expected IO and pre-parsed globals excluded; output drop and validation excluded",
            })
        );
        return Ok(());
    }
    if args[0] == "alloc" {
        #[cfg(feature = "allocations")]
        {
            let mut output = None;
            let info = allocation_counter::measure(|| {
                output = Some(operation());
            });
            let output = output.ok_or("allocation closure did not execute")??;
            validate(&output, case, &expected, global_count)?;
            println!(
                "{}",
                serde_json::json!({
                    "id":case.id, "mode":mode, "scope":"one warm-input operation, result held at end; existing input/expected/globals outside tracking",
                    "allocations_total":info.count_total,"allocations_peak":info.count_max,
                    "bytes_total":info.bytes_total,"bytes_peak":info.bytes_max,"bytes_current":info.bytes_current,
                    "exact_output_checked":true,
                })
            );
            return Ok(());
        }
        #[cfg(not(feature = "allocations"))]
        return Err(
            "rebuild with --features allocations for tracking (never use this binary for timings)"
                .into(),
        );
    }
    if args[0] == "profile" {
        let repetitions: usize = args[5].parse()?;
        let output = profile_run(&mut operation, repetitions)?;
        validate(&output, case, &expected, global_count)?;
        println!(
            "{}",
            serde_json::json!({"id":case.id,"mode":mode,"repetitions":repetitions,"exact_output_checked":true})
        );
        return Ok(());
    }
    if args[0] != "measure" {
        return Err("unknown action".into());
    }
    let samples: usize = args[4].parse()?;
    let target_ms: u64 = args[5].parse()?;
    if !(5..=100).contains(&samples) || !(10..=5000).contains(&target_ms) {
        return Err("samples must be5..100 and target milliseconds10..5000".into());
    }
    for _ in 0..3 {
        let output = operation()?;
        validate(&output, case, &expected, global_count)?;
        drop(black_box(output));
    }
    let start = Instant::now();
    let pilot = operation()?;
    let pilot_ns = start.elapsed().as_nanos().max(1);
    validate(&pilot, case, &expected, global_count)?;
    drop(black_box(pilot));
    let batch =
        (u128::from(target_ms) * 1_000_000 / samples as u128 / pilot_ns).clamp(1, 2000) as usize;
    let mut raw = Vec::with_capacity(samples);
    for _ in 0..samples {
        let mut elapsed = 0;
        for _ in 0..batch {
            let start = Instant::now();
            let output = operation()?;
            elapsed += start.elapsed().as_nanos();
            validate(&output, case, &expected, global_count)?;
            drop(black_box(output));
        }
        raw.push(elapsed / batch as u128);
    }
    let mut ordered = raw.clone();
    ordered.sort_unstable();
    println!(
        "{}",
        serde_json::json!({
            "id":case.id,"family":case.family,"mode":mode,"policy":case.policy,
            "width":case.width,"height":case.height,"pages":case.pages,
            "input_bytes":input.len(),"globals_bytes":globals_bytes.as_ref().map_or(0,Vec::len),
            "output_bytes":expected.len()*case.pages,"pixels":u64::from(case.width)*u64::from(case.height)*case.pages as u64,
            "samples":samples,"batch":batch,"samples_ns":raw,
            "median_ns":ordered[ordered.len()/2],"p95_ns":ordered[(ordered.len()*95).div_ceil(100)-1],
            "peak_rss_kib":peak_rss()?,"recoveries":recoveries,"exact_output_checked":true,
            "scope":"decode/inspect operation and returned ownership; file IO, globals parsing(except explicit modes), validation/hash and output drop outside timings",
        })
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_gate_rejects_one_wrong_pixel_and_wrong_dimensions() {
        let generated = generated_cases::cases(Some("white-64")).pop().unwrap();
        let output = Output::Document(Decoder::default().decode(&generated.input).unwrap());
        let mut case = Case {
            id: "control".into(),
            family: "mmr".into(),
            input: String::new(),
            globals: None,
            expected: String::new(),
            width: 64,
            height: 64,
            pages: 1,
            policy: "strict".into(),
            standalone: true,
        };
        let mut expected = vec![0; 512];
        validate(&output, &case, &expected, 0).unwrap();
        expected[0] = 0x80;
        assert!(validate(&output, &case, &expected, 0).is_err());
        expected[0] = 0;
        case.width = 63;
        assert!(validate(&output, &case, &expected, 0).is_err());
    }

    #[test]
    fn output_gate_rejects_silently_dropped_pages() {
        let generated = generated_cases::cases(Some("many-pages-32")).pop().unwrap();
        let output = Output::Document(Decoder::default().decode(&generated.input).unwrap());
        let mut case = Case {
            id: "control".into(),
            family: "mmr".into(),
            input: String::new(),
            globals: None,
            expected: String::new(),
            width: 64,
            height: 64,
            pages: 32,
            policy: "strict".into(),
            standalone: true,
        };
        validate(&output, &case, &[0; 512], 0).unwrap();
        case.pages = 1;
        assert!(validate(&output, &case, &[0; 512], 0).is_err());
    }
}
