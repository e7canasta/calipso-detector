//! The detector over a directory of frames, without a camera, without waiting
//! and without NATS: the same driver, senses and models as the camera
//! binary, each frame a sample, none dropped. Writes what it would publish,
//! one message per line (`{"subject", "body"}`), to compare two builds over
//! the same clip frame by frame (white book, volume VII, ch. 12).
use anyhow::{Context, Result};
use clap::Parser;
use detector::source::frames::FramesSource;
use detector::{
    build, clock::SystemClock, config::Config, driver::Driver, publish::PublisherHandle,
};
use scene_contract::Source;
use std::{io::Write, path::PathBuf, time::Duration};
use tokio_util::sync::CancellationToken;

/// The stall watchdog's timeout over frames: a year, never in a test.
const NEVER_STALLS: Duration = Duration::from_secs(365 * 24 * 3600);

#[derive(Debug, Parser)]
#[command(
    version,
    about = "The detector over a directory of frames, for the loop test"
)]
struct Args {
    /// The camera profile; its `[stream]` is not used.
    #[arg(short, long)]
    config: PathBuf,
    /// JPEG frames named by their source second (`12.jpg`).
    #[arg(long)]
    frames: PathBuf,
    /// Where to write each published message.
    #[arg(long)]
    out: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let args = Args::parse();
    let config = Config::load(&args.config)?;
    let profile = config.profile()?;
    anyhow::ensure!(
        config.senses.verify.is_none(),
        "drop [senses.verify]: the verifier is not repeatable"
    );
    let mut source = FramesSource::open(&args.frames, SystemClock)
        .with_context(|| format!("read {}", args.frames.display()))?;
    anyhow::ensure!(
        !source.is_empty(),
        "no frames named by their second in {}",
        args.frames.display()
    );
    let run = Source {
        site: config.camera.site.clone(),
        camera: config.camera.id.clone(),
        run_id: ulid::Ulid::new().to_string(),
        build: build::current(),
    };
    tracing::info!(run_id = %run.run_id, build = ?run.build, frames = source.len(), "runs over frames");
    let prefix = config.prefix();
    let (mut publisher, _rx) = PublisherHandle::channel(run, profile);
    let mut file = std::fs::File::create(&args.out)
        .with_context(|| format!("create {}", args.out.display()))?;
    let senses = detector::senses::load(&config)?;
    let mut driver = Driver::new(
        config.policy()?,
        config.site_map(),
        senses,
        // No camera, nothing to stall: the watchdog runs on the wall clock,
        // and a slow turn on a loaded machine must not change the scenes.
        NEVER_STALLS,
        SystemClock,
    );
    let mut error = None;
    driver.run(&mut source, &CancellationToken::new(), |processed| {
        let written = publisher
            .prepare(processed.output, processed.timing)
            .messages()
            .map_err(anyhow::Error::from)
            .and_then(|messages| {
                for m in messages {
                    let body = std::str::from_utf8(&m.bytes)?;
                    writeln!(
                        file,
                        r#"{{"subject":"{}","body":{body}}}"#,
                        m.kind.subject(&prefix)
                    )?;
                }
                Ok(())
            });
        if let Err(e) = written {
            error.get_or_insert(e);
        }
    });
    error.map_or(Ok(()), Err)
}
