//! Per-camera composition: each resource is born and dies in its thread.
use anyhow::{Context, Result};
use clap::Parser;
use detector::{
    build,
    config::Config,
    driver::Driver,
    publish::{PublisherHandle, nats},
    source::rtsp::RtspSource,
};
use ingest::{Mailbox, Sample};
use scene_core::contract::Source;
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
    /// Runs a golden scene instead of the camera: the source is the scene and
    /// the senses are what it says they see. Opens no camera or models.
    #[arg(long)]
    scene: Option<PathBuf>,
    /// With --scene: without waiting between samples. Requires --jsonl (does not
    /// publish to NATS).
    #[arg(long, requires = "scene", requires = "jsonl")]
    fast: bool,
    /// With --scene: writes to this file the same it publishes, each line with
    /// its `schema`, for `calipso-rerun-bridge --replay`.
    #[arg(long, requires = "scene")]
    jsonl: Option<PathBuf>,
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
    if let Some(path) = &args.scene {
        return run_scene(config, profile, path, args.fast, args.jsonl.as_deref()).await;
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
    // Ingest keeps its watch API; a single cancellation feeds both signals.
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
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
        shutdown_rx,
    );
    let mut engine = tokio::task::spawn_blocking({
        let mailbox = Arc::clone(&mailbox);
        let token = token.clone();
        move || -> Result<()> {
            let detect = sense::ultralytics::Detector::load(
                &config.model_path(&config.senses.detect.model),
                config.senses.detect.backend_confidence,
            )
            .context("load ONNX detector")?;
            let mut senses = sense::Senses::new(detect);
            if let Some(reinforce) = &config.senses.reinforce {
                let path = config.model_path(&reinforce.model);
                tracing::info!(model = %path.display(), "loads ONNX reinforcement");
                let model = sense::ultralytics::Detector::load(&path, reinforce.backend_confidence)
                    .with_context(|| format!("load ONNX reinforcement: {}", path.display()))?;
                senses = senses.with_reinforce(model);
            }
            if let Some(faces) = &config.senses.faces {
                let path = config.model_path(&faces.model);
                tracing::info!(model = %path.display(), "loads ONNX faces");
                let model = sense::ultralytics::Detector::load(&path, faces.backend_confidence)
                    .with_context(|| format!("load ONNX faces: {}", path.display()))?;
                senses = senses.with_faces(model);
            }
            if let Some(pose) = &config.senses.pose {
                let path = config.model_path(&pose.model);
                tracing::info!(model = %path.display(), "loads ONNX pose");
                let model = sense::ultralytics::PoseEstimator::load(&path, pose.backend_confidence)
                    .with_context(|| format!("load ONNX pose: {}", path.display()))?;
                senses = senses.with_pose(model);
            }
            if let Some(depth) = &config.senses.depth {
                let path = config.model_path(&depth.model);
                tracing::info!(model = %path.display(), "loads ONNX depth");
                let model = sense::ultralytics::DepthEstimator::load(&path)
                    .with_context(|| format!("load ONNX depth: {}", path.display()))?;
                senses = senses.with_depth(model);
            }
            let mut driver = Driver::new(
                config.policy(),
                config.site_map(),
                senses,
                Duration::from_secs(config.stream.stall_timeout_secs),
            );
            let mut source = RtspSource::new(
                mailbox,
                events_rx,
                Duration::from_millis(config.stream.sample_interval_ms),
            );
            driver.run(&mut source, &token, |processed| {
                publisher.send(processed.output, processed.timing);
            });
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
    shutdown_tx.send_replace(true);
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

/// The replay engine: the same composition as live, with the scene's
/// source and the scripted senses instead of RTSP and the models. The core, the
/// driver and the publisher are the usual ones.
async fn run_scene(
    config: Config,
    profile: scene_core::contract::Profile,
    path: &std::path::Path,
    fast: bool,
    jsonl: Option<&std::path::Path>,
) -> Result<()> {
    use detector::senses::Scripted;
    use detector::source::scene::{Pace, SceneSource};
    use golden::{
        expected::Verification,
        language::{Scene, Sense},
        world,
    };
    use std::io::Write;

    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let scene = Scene::parse(&text).with_context(|| format!("scene {}", path.display()))?;
    let site = config.site_map();
    anyhow::ensure!(
        site.resolution == Some(world::FRAME)
            && site.bed == scene_core::geom::Quad::from_rect(world::BED),
        "the profile does not have the scenes' geometry (bed 100,100 to 500,400 in 640×480): use config/scene.toml"
    );
    for (sense, has, name) in [
        (
            Sense::Reinforce,
            config.senses.reinforce.is_some(),
            "reinforcement",
        ),
        (Sense::Faces, config.senses.faces.is_some(), "faces"),
    ] {
        anyhow::ensure!(
            !scene.senses.contains(&sense) || has,
            "the scene assumes {name} and the profile does not have it: use config/scene-senses.toml"
        );
    }
    if let Some(dwell) = scene.dwell_s
        && (dwell - config.policy().persistence.dwell_secs).abs() > f64::EPSILON
    {
        tracing::warn!(
            scene = dwell,
            profile = config.policy().persistence.dwell_secs,
            "the scene's expectations hold with another dwell"
        );
    }
    let name = path
        .file_stem()
        .map_or_else(|| "scene".into(), |s| s.to_string_lossy().into_owned());
    let source = Source {
        site: config.camera.site.clone(),
        camera: config.camera.id.clone(),
        run_id: format!("{name}-{}", ulid::Ulid::new()),
        build: build::current(),
    };
    tracing::info!(scene = %scene.title, run_id = %source.run_id, fast, "runs scene");
    let (mut publisher, rx) = PublisherHandle::channel(source, profile);
    let token = CancellationToken::new();
    let actor = (!fast).then(|| {
        tokio::spawn(nats::run(
            config.nats.clone(),
            config.prefix(),
            rx,
            token.clone(),
        ))
    });
    let mut file = jsonl
        .map(|p| std::fs::File::create(p).with_context(|| format!("create {}", p.display())))
        .transpose()?;
    let pace = if fast { Pace::Fast } else { Pace::Real };
    let stall = Duration::from_secs(config.stream.stall_timeout_secs);
    let engine = tokio::task::spawn_blocking({
        let token = token.clone();
        move || -> Result<Verification> {
            let mut driver = Driver::new(config.policy(), config.site_map(), Scripted, stall);
            let mut source = SceneSource::new(&scene, pace);
            let mut verification = Verification::new(&scene);
            let mut error = None;
            driver.run(&mut source, &token, |processed| {
                if processed.output.scene.republished.is_none()
                    && let Ok(n) = u32::try_from(processed.output.at.sequence)
                {
                    verification.observe(n, &processed.output.scene);
                }
                let publication = publisher.prepare(processed.output, processed.timing);
                if let Some(file) = file.as_mut() {
                    let written = publication
                        .messages()
                        .map_err(anyhow::Error::from)
                        .and_then(|ms| {
                            ms.iter().try_for_each(|m| {
                                file.write_all(&m.bytes)?;
                                file.write_all(b"\n")
                            })?;
                            Ok(())
                        });
                    if let Err(e) = written {
                        error.get_or_insert(e);
                    }
                }
                if !fast {
                    publisher.publish(publication);
                }
            });
            match error {
                Some(e) => Err(e).context("write JSONL"),
                None => Ok(verification),
            }
        }
    });
    let verification = engine.await.context("wait for the scene engine")??;
    if let Some(actor) = actor {
        // Give the queue time to drain before closing the network.
        tokio::time::sleep(Duration::from_millis(500)).await;
        token.cancel();
        actor.await.context("wait for the publisher")?;
    }
    for o in verification.outcomes() {
        let span = format!("{}-{}", o.samples.from, o.samples.to);
        match &o.difference {
            None => println!("✓ span {span}"),
            Some(d) => println!("✗ span {span}: {d}"),
        }
    }
    anyhow::ensure!(
        verification.matches(),
        "the scene does not match its expectations"
    );
    Ok(())
}
