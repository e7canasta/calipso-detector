//! Per-camera composition: each resource is born and dies in its thread.
use anyhow::{Context, Result};
use clap::Parser;
use detector::{
    build,
    clock::SystemClock,
    config::Config,
    driver::Driver,
    publish::{PublisherHandle, nats},
    source::rtsp::RtspSource,
    verifier::{DeadlineVerifier, NatsVerifier},
};
use ingest::{Mailbox, Sample};
use scene_contract::Source;
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Duration,
};
use tokio::signal::unix::{SignalKind, signal};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Parser)]
#[command(version, about = "Per-camera scene detector")]
struct Args {
    #[arg(short, long)]
    config: PathBuf,
    /// Validates the configuration and prints the profile without opening camera or models.
    #[arg(long)]
    check: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Only this binary: a panic ends the whole process, in debug too. The
    // workspace profile belongs to other specs and is not changed here.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        std::process::abort();
    }));
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let args = Args::parse();
    let config = Config::load(&args.config)?;
    let profile = config.profile()?;
    if args.check {
        println!("{}", serde_json::to_string(&profile)?);
        return Ok(());
    }

    // Install before creating threads: if it fails there is nothing to close.
    let mut terminate = signal(SignalKind::terminate()).context("install SIGTERM")?;
    let source = Source {
        site: config.camera.site.clone(),
        camera: config.camera.id.clone(),
        run_id: ulid::Ulid::new().to_string(),
        build: build::current(),
    };
    tracing::info!(run_id=%source.run_id,build=?source.build,profile=?profile,"detector starts");
    let (mut publisher, rx) = PublisherHandle::channel(source, profile);
    let token = CancellationToken::new();
    let mailbox = Arc::new(Mailbox::<Sample>::default());
    let (events_tx, events_rx) = mpsc::channel();
    let actor = tokio::spawn(nats::run(
        config.nats.clone(),
        config.prefix(),
        rx,
        token.clone(),
    ));
    let stream = ingest::spawn(
        config.ingest(),
        Arc::clone(&mailbox),
        events_tx,
        token.clone(),
        SystemClock,
    );
    // The slow verifier, if the profile has one: a local service behind NATS,
    // on its own connection, outside the turn.
    let verifier = config.senses.verify.as_ref().map(|sense| {
        DeadlineVerifier::new(
            NatsVerifier::spawn(
                &tokio::runtime::Handle::current(),
                config.nats.url.clone(),
                sense.subject.clone(),
                config.prefix(),
                config.camera.id.clone(),
            ),
            Duration::from_secs_f64(sense.wall_timeout_secs),
            SystemClock,
        )
    });
    let mut engine = tokio::task::spawn_blocking({
        let mailbox = Arc::clone(&mailbox);
        let token = token.clone();
        move || -> Result<()> {
            let senses = detector::senses::load(&config)?;
            let driver = Driver::new(
                config.policy()?,
                config.site_map(),
                senses,
                Duration::from_secs(config.stream.stall_timeout_secs),
                SystemClock,
            );
            let mut source = RtspSource::new(
                mailbox,
                events_rx,
                Duration::from_millis(config.stream.sample_interval_ms),
                SystemClock,
            );
            let emit = |processed: detector::driver::Processed| {
                publisher.send(processed.output, processed.timing);
            };
            match verifier {
                Some(verifier) => driver
                    .with_verifier(verifier)
                    .run(&mut source, &token, emit),
                None => {
                    let mut driver = driver;
                    driver.run(&mut source, &token, emit);
                }
            }
            Ok(())
        }
    });
    let mut signal_error = None;
    let result = tokio::select! {
        result=&mut engine => Some(result),
        result=tokio::signal::ctrl_c() => {signal_error=result.err();tracing::info!("SIGINT; detector closes");None},
        _=terminate.recv() => {tracing::info!("SIGTERM; detector closes");None},
    };
    token.cancel();
    mailbox.close();
    let result = match result {
        Some(result) => result,
        None => engine.await,
    };
    let stream_result = tokio::task::spawn_blocking(move || stream.join()).await;
    let actor_result = actor.await;
    if let Some(error) = signal_error {
        return Err(error).context("wait for SIGINT");
    }
    stream_result
        .context("wait for the ingest thread")?
        .map_err(|_| anyhow::anyhow!("ingest thread failed"))?;
    actor_result.context("wait for the publisher")?;
    result.context("wait for the engine thread")??;
    Ok(())
}
