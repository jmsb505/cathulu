use cathulu_core::{default_data_dir, Store};
use std::io::{self, Write};

fn main() {
    if let Err(error) = run() {
        eprintln!("Cathulu maintenance failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;
    let store = Store::open(default_data_dir()?)?;
    match command.as_str() {
        "gardener" => {
            let mut minimum_support = 2;
            if let Some(option) = args.next() {
                if option != "--minimum-support" {
                    return Err(usage());
                }
                minimum_support = args
                    .next()
                    .ok_or_else(usage)?
                    .parse()
                    .map_err(|_| "minimum support must be an integer".to_string())?;
            }
            reject_extra(args)?;
            let report = store.run_memory_gardener(minimum_support)?;
            print_json(&report)
        }
        "evidence-gc" => {
            let action = args.next().ok_or_else(usage)?;
            match action.as_str() {
                "preview" => {
                    reject_extra(args)?;
                    print_json(&store.preview_evidence_collection()?)
                }
                "collect" => {
                    reject_extra(args)?;
                    let preview = store.preview_evidence_collection()?;
                    let eligible_ids = preview
                        .iter()
                        .filter(|item| item.eligible)
                        .map(|item| item.artifact_id)
                        .collect::<Vec<_>>();
                    let eligible = eligible_ids.len();
                    print_json(&preview)?;
                    if eligible == 0 {
                        println!("No evidence references are currently eligible for archiving.");
                        return Ok(());
                    }
                    print!("Type ARCHIVE {eligible} to archive Cathulu's eligible local URI references: ");
                    io::stdout()
                        .flush()
                        .map_err(|error| format!("cannot display confirmation prompt: {error}"))?;
                    let mut confirmation = String::new();
                    io::stdin()
                        .read_line(&mut confirmation)
                        .map_err(|error| format!("cannot read confirmation: {error}"))?;
                    if confirmation.trim() != format!("ARCHIVE {eligible}") {
                        return Err(
                            "confirmation did not match; no evidence references were archived"
                                .to_string(),
                        );
                    }
                    print_json(&store.collect_evidence_references(&eligible_ids)?)
                }
                "set-retention" => {
                    let age = args.next().ok_or_else(usage)?.parse::<i64>().map_err(|_| {
                        "retention age must be a non-negative number of seconds".to_string()
                    })?;
                    reject_extra(args)?;
                    if age < 0 {
                        return Err("retention age must be non-negative".to_string());
                    }
                    print_json(&store.set_evidence_retention_policy(Some(age), true)?)
                }
                "disable-retention" => {
                    reject_extra(args)?;
                    print_json(&store.set_evidence_retention_policy(None, true)?)
                }
                "mark-extracted" => {
                    let artifact_id = parse_id(args.next().ok_or_else(usage)?, "artifact ID")?;
                    let memory_ids = args
                        .map(|value| parse_id(value, "memory ID"))
                        .collect::<Result<Vec<_>, _>>()?;
                    print_json(&serde_json::json!({
                        "receiptId": store.record_evidence_extraction(artifact_id, &memory_ids)?,
                        "artifactId": artifact_id
                    }))
                }
                "hold" => {
                    let artifact_id = parse_id(args.next().ok_or_else(usage)?, "artifact ID")?;
                    let reason = args.collect::<Vec<_>>().join(" ");
                    print_json(
                        &serde_json::json!({ "holdId": store.hold_evidence(artifact_id, &reason)? }),
                    )
                }
                "release-hold" => {
                    let hold_id = parse_id(args.next().ok_or_else(usage)?, "hold ID")?;
                    reject_extra(args)?;
                    print_json(
                        &serde_json::json!({ "released": store.release_evidence_hold(hold_id)? }),
                    )
                }
                _ => Err(usage()),
            }
        }
        _ => Err(usage()),
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), String> {
    let output = serde_json::to_string_pretty(value)
        .map_err(|error| format!("cannot encode maintenance result: {error}"))?;
    println!("{output}");
    Ok(())
}

fn parse_id(value: String, label: &str) -> Result<i64, String> {
    let id = value
        .parse::<i64>()
        .map_err(|_| format!("{label} must be a positive integer"))?;
    if id <= 0 {
        return Err(format!("{label} must be a positive integer"));
    }
    Ok(id)
}

fn reject_extra(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    if args.next().is_some() {
        Err(usage())
    } else {
        Ok(())
    }
}

fn usage() -> String {
    "Usage: cathulu-maintain gardener [--minimum-support N] | evidence-gc <preview|collect|set-retention SECONDS|disable-retention|mark-extracted ARTIFACT_ID MEMORY_ID...|hold ARTIFACT_ID REASON|release-hold HOLD_ID>".to_string()
}
