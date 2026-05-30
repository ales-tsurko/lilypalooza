//! Isolated plugin validation helper.

use std::{io, path::PathBuf};

/// Structured validation report for any supported plugin format.
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum ValidationReport {
    /// AUv2 validation report.
    Au(lilypalooza_au::ValidationReport),
    /// CLAP validation report.
    Clap(lilypalooza_clap::ValidationReport),
    /// VST3 validation report.
    Vst3(lilypalooza_vst3::ValidationReport),
}

/// Runs validator CLI logic, writes CLI output, and returns a process exit code.
pub fn run_cli(args: Vec<String>) -> i32 {
    match run(args) {
        Ok(report) => match serde_json::to_writer(io::stdout(), &report) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("failed to write validation report: {error}");
                2
            }
        },
        Err(error) => {
            eprintln!("{error}");
            2
        }
    }
}

/// Runs validator CLI logic and returns a structured report.
pub fn run(args: Vec<String>) -> Result<ValidationReport, String> {
    let args = parse_args(args)?;
    validate_path(args)
}

struct ValidatorArgs {
    format: String,
    path: PathBuf,
}

#[derive(Debug, Clone, Copy)]
enum ValidatorFormat {
    Au,
    Clap,
    Vst3,
}

impl ValidatorFormat {
    fn parse(format: &str) -> Result<Self, String> {
        match format {
            lilypalooza_au::FORMAT => Ok(Self::Au),
            lilypalooza_clap::FORMAT => Ok(Self::Clap),
            lilypalooza_vst3::FORMAT => Ok(Self::Vst3),
            _ => Err(format!("unsupported plugin format: {format}")),
        }
    }

    fn is_supported_on_current_platform(self) -> bool {
        match self {
            Self::Au => cfg!(target_os = "macos"),
            Self::Clap | Self::Vst3 => true,
        }
    }
}

fn parse_args(args: Vec<String>) -> Result<ValidatorArgs, String> {
    let mut format = None;
    let mut path = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--format" => format = iter.next(),
            "--path" => path = iter.next().map(PathBuf::from),
            "--help" | "-h" => return Err(usage()),
            other => return Err(format!("unknown argument: {other}\n{}", usage())),
        }
    }

    let format = format.ok_or_else(usage)?;
    let path = path.ok_or_else(usage)?;
    Ok(ValidatorArgs { format, path })
}

fn validate_path(args: ValidatorArgs) -> Result<ValidationReport, String> {
    let format = ValidatorFormat::parse(&args.format)?;
    if !format.is_supported_on_current_platform() {
        return Err(format!(
            "unsupported plugin format on this platform: {}",
            args.format
        ));
    }

    match format {
        ValidatorFormat::Au => {
            let result = lilypalooza_au::probe(&args.path).map_err(|error| error.to_string());
            Ok(ValidationReport::Au(lilypalooza_au::ValidationReport {
                format: args.format,
                path: args.path,
                result,
            }))
        }
        ValidatorFormat::Clap => {
            let result = lilypalooza_clap::probe(&args.path).map_err(|error| error.to_string());
            Ok(ValidationReport::Clap(lilypalooza_clap::ValidationReport {
                format: args.format,
                path: args.path,
                result,
            }))
        }
        ValidatorFormat::Vst3 => {
            let result = lilypalooza_vst3::probe(&args.path).map_err(|error| error.to_string());
            Ok(ValidationReport::Vst3(lilypalooza_vst3::ValidationReport {
                format: args.format,
                path: args.path,
                result,
            }))
        }
    }
}

fn usage() -> String {
    "usage: lilypalooza-plugin-validator --format au|clap|vst3 --path <plugin>".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_format() {
        let error = run(vec![
            "--format".to_string(),
            "vst2".to_string(),
            "--path".to_string(),
            "plugin".to_string(),
        ])
        .expect_err("unknown format should fail");

        assert!(error.contains("unsupported plugin format: vst2"));
    }

    #[test]
    fn requires_format_and_path() {
        let missing_format =
            run(vec!["--path".to_string(), "/tmp/plugin".to_string()]).unwrap_err();
        let missing_path = run(vec!["--format".to_string(), "au".to_string()]).unwrap_err();

        assert!(missing_format.contains("usage:"));
        assert!(missing_path.contains("usage:"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn validates_au_path_as_structured_report() {
        let report = run(vec![
            "--format".to_string(),
            "au".to_string(),
            "--path".to_string(),
            "/tmp/missing.component".to_string(),
        ])
        .unwrap();

        match report {
            ValidationReport::Au(report) => {
                assert_eq!(report.format, "au");
                let error = report.result.unwrap_err();
                assert!(error.contains("Info.plist"));
            }
            ValidationReport::Clap(_) | ValidationReport::Vst3(_) => {
                panic!("expected AU report");
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn rejects_au_on_non_macos() {
        let error = run(vec![
            "--format".to_string(),
            "au".to_string(),
            "--path".to_string(),
            "/tmp/plugin.component".to_string(),
        ])
        .unwrap_err();

        assert!(error.contains("unsupported plugin format on this platform"));
    }
}
