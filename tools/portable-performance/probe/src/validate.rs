//! Correctness only: no timers, profiling, adaptive batches or golden writes.
use std::fs;
include!("api.rs");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err("usage: api-validation MANIFEST".into());
    }
    let cases: Vec<Case> = serde_json::from_slice(&fs::read(&args[0])?)?;
    let mut result = Vec::new();
    for case in cases {
        let input = fs::read(&case.input)?;
        let expected = fs::read(&case.expected)?;
        let decoder = Decoder::default();
        let globals_bytes = case.globals.as_ref().map(fs::read).transpose()?;
        let globals = globals_bytes
            .as_ref()
            .map(|bytes| decoder.parse_globals(bytes))
            .transpose()?;
        let global_count = globals.as_ref().map_or(0, GlobalSegments::segment_count);
        let mut context = decoder.context();
        let mut modes = if case.standalone {
            vec!["fresh", "context", "inspect", "reader"]
        } else if case.policy == "strict" {
            vec!["fresh", "context", "inspect", "policy-strict"]
        } else {
            vec!["best-effort", "best-effort-context"]
        };
        if globals.is_some() {
            modes.extend(["parse-globals", "reparse-globals"]);
        }
        for mode in modes {
            eprintln!("API correctness case: {} mode: {}", case.id, mode);
            let mut last_recoveries = String::new();
            for _ in 0..2 {
                let policy = if mode.starts_with("best-effort") {
                    EmbeddedPolicy::BestEffort
                } else {
                    EmbeddedPolicy::Strict
                };
                let output = match mode {
                    "fresh" if case.standalone => Output::Document(decoder.decode(&input)?),
                    "fresh" => Output::Document(match &globals {
                        Some(globals) => decoder.decode_embedded_with_globals(&input, globals)?,
                        None => decoder.decode_embedded(&input)?,
                    }),
                    "context" if case.standalone => Output::Document(context.decode(&input)?),
                    "context" => Output::Document(match &globals {
                        Some(globals) => context.decode_embedded_with_globals(&input, globals)?,
                        None => context.decode_embedded(&input)?,
                    }),
                    "inspect" if case.standalone => Output::Info(decoder.inspect(&input)?),
                    "inspect" => Output::Info(match &globals {
                        Some(globals) => decoder.inspect_embedded_with_globals(&input, globals)?,
                        None => decoder.inspect_embedded(&input)?,
                    }),
                    "reader" => Output::Document(decoder.decode_reader(&mut input.as_slice())?),
                    "best-effort" | "policy-strict" => {
                        let output = decoder.decode_embedded_with_policy(
                            &input,
                            globals.as_ref(),
                            policy,
                        )?;
                        Output::Recovered(output.value, output.recoveries)
                    }
                    "best-effort-context" => {
                        let output = context.decode_embedded_with_policy(
                            &input,
                            globals.as_ref(),
                            policy,
                        )?;
                        Output::Recovered(output.value, output.recoveries)
                    }
                    "parse-globals" => Output::Globals(
                        decoder.parse_globals(globals_bytes.as_ref().ok_or("missing globals")?)?,
                    ),
                    "reparse-globals" => {
                        let state = decoder
                            .parse_globals(globals_bytes.as_ref().ok_or("missing globals")?)?;
                        Output::Document(decoder.decode_embedded_with_globals(&input, &state)?)
                    }
                    _ => return Err("unsupported validation mode".into()),
                };
                validate(&output, &case, &expected, global_count)?;
                let recoveries = match output {
                    Output::Recovered(_, recoveries) => format!("{recoveries:?}"),
                    _ => "[]".into(),
                };
                if !last_recoveries.is_empty() && last_recoveries != recoveries {
                    return Err("reused context changed recovery metadata".into());
                }
                last_recoveries = recoveries;
            }
            result.push(serde_json::json!({
                "id":case.id,"mode":mode,"policy":case.policy,"pages":case.pages,
                "width":case.width,"height":case.height,"recoveries":last_recoveries,
                "globals":global_count,"exact_output_checked":true,"repetitions":2,
            }));
        }
    }
    if result.len() != 492 {
        return Err(format!("do not lower correctness denominator: {}", result.len()).into());
    }
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
