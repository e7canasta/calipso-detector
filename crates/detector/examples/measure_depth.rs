//! Measures the bed's depth: the five points calibration asks for
//! (`SiteMap::bed_points`: four corners and the centre) in each sample of the
//! profile's RTSP, with the depth model, split by span (empty bed, occupied and
//! seen, occupied unseen). No pixels (rule 7): only numbers. Whoever measures
//! gives the span (`--empty-until`, `--occupied-from`, in source seconds since
//! the first sample), for example from a golden scene; the main detector only
//! says whether it sees someone in bed.
use anyhow::{Context, Result, ensure};
use clap::Parser;
use detector::{
    config::Config,
    source::{Arrival, SampleSource, rtsp::RtspSource},
};
use ingest::{Mailbox, Sample};
use scene_core::{
    geom::Point,
    look::{Label, Probe, Reading},
};
use sense::{DepthPort, Senses, ultralytics::DepthEstimator};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    io::Write as _,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

#[derive(Debug, Parser)]
struct Args {
    /// The camera profile: RTSP, bed and main model.
    #[arg(long)]
    config: PathBuf,
    /// The depth model, if the profile has no `[senses.depth]`.
    #[arg(long, default_value = "yolo26s-depth.onnx")]
    depth_model: PathBuf,
    #[arg(long, default_value_t = 300)]
    seconds: u64,
    /// The bed is empty up to this source second.
    #[arg(long)]
    empty_until: f64,
    /// Someone is lying down from this source second.
    #[arg(long)]
    occupied_from: f64,
    /// One line per sample, numbers only: second, span, values.
    #[arg(long)]
    jsonl: Option<PathBuf>,
}

#[derive(Debug, Default)]
struct Span {
    /// Per point (0–4) and the median of the five (5), like the core's reference.
    points: [Vec<f64>; 6],
    /// The depth at the centre of the box of whoever is in bed.
    body: Vec<f64>,
}

fn stats(v: &[f64]) -> Option<(f64, f64, f64, f64)> {
    if v.is_empty() {
        return None;
    }
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
    let min = v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some((mean, sd, min, max))
}

fn median(values: &[f64]) -> Option<f64> {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    match v.len() {
        0 => None,
        n if n % 2 == 1 => Some(v[n / 2]),
        n => Some((v[n / 2 - 1] + v[n / 2]) / 2.0),
    }
}

fn quantile(values: &[f64], q: f64) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    v[((q * v.len() as f64).ceil().max(1.0) as usize - 1).min(v.len() - 1)]
}

const NAMES: [&str; 6] = [
    "corner 1",
    "corner 2",
    "corner 3",
    "corner 4",
    "centre",
    "median of the 5",
];
const SPANS: [&str; 4] = ["empty", "occupied, seen", "occupied, unseen", "transition"];

fn report(
    spans: &BTreeMap<&'static str, Span>,
    points: &[Point],
    scale: &[(f64, f64, f64)],
    times: &[f64],
) -> String {
    let mut out = String::new();
    writeln!(
        out,
        "Bed points (frame): {}.\n",
        points
            .iter()
            .map(|p| format!("({:.0},{:.0})", p.x, p.y))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .unwrap();
    out.push_str("| Span | Point | n | mean | sd | min | max |\n| --- | --- | ---: | ---: | ---: | ---: | ---: |\n");
    for name in SPANS {
        let Some(s) = spans.get(name) else {
            continue;
        };
        for (i, point) in NAMES.iter().enumerate() {
            if let Some((m, sd, lo, hi)) = stats(&s.points[i]) {
                writeln!(
                    out,
                    "| {name} | {point} | {} | {m:.4} | {sd:.4} | {lo:.4} | {hi:.4} |",
                    s.points[i].len()
                )
                .unwrap();
            }
        }
        if let Some((m, sd, lo, hi)) = stats(&s.body) {
            writeln!(
                out,
                "| {name} | body centre | {} | {m:.4} | {sd:.4} | {lo:.4} | {hi:.4} |",
                s.body.len()
            )
            .unwrap();
        }
    }
    if let Some(empty) = spans.get("empty") {
        out.push_str("\nDifference of means against the empty bed, and in empty-bed standard deviations:\n\n| Span | Point | Δ mean | Δ / empty sd |\n| --- | --- | ---: | ---: |\n");
        for name in ["occupied, seen", "occupied, unseen"] {
            let Some(s) = spans.get(name) else {
                continue;
            };
            for (i, point) in NAMES.iter().enumerate() {
                if let (Some((me, sde, _, _)), Some((mo, _, _, _))) =
                    (stats(&empty.points[i]), stats(&s.points[i]))
                {
                    writeln!(
                        out,
                        "| {name} | {point} | {:+.4} | {:+.1} |",
                        mo - me,
                        (mo - me) / sde.max(f64::EPSILON)
                    )
                    .unwrap();
                }
            }
        }
    }
    if !scale.is_empty() {
        let mins: Vec<f64> = scale.iter().map(|e| e.0).collect();
        let p50s: Vec<f64> = scale.iter().map(|e| e.1).collect();
        let maxs: Vec<f64> = scale.iter().map(|e| e.2).collect();
        writeln!(out, "\nWhole-map scale (per sample; mean of min / median / max): {:.4} / {:.4} / {:.4}; absolute min {:.4}, absolute max {:.4}.", stats(&mins).unwrap().0, stats(&p50s).unwrap().0, stats(&maxs).unwrap().0, stats(&mins).unwrap().2, stats(&maxs).unwrap().3).unwrap();
    }
    if !times.is_empty() {
        writeln!(
            out,
            "\nDepth time over the whole frame (n={}): p50 {:.3} ms, p90 {:.3} ms, max {:.3} ms.",
            times.len(),
            quantile(times, 0.5),
            quantile(times, 0.9),
            quantile(times, 1.0)
        )
        .unwrap();
    }
    out
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        args.empty_until < args.occupied_from,
        "--empty-until must come before --occupied-from"
    );
    let config = Config::load(&args.config)?;
    let site = config.site_map();
    let points = site.calibration_points();
    let depth_model = config.model_path(
        config
            .senses
            .depth
            .as_ref()
            .map_or(&args.depth_model, |d| &d.model),
    );
    let detect = sense::ultralytics::Detector::load(
        &config.model_path(&config.senses.detect.model),
        config.senses.detect.backend_confidence,
    )
    .context("load the main detector")?;
    let mut depth = DepthEstimator::load(&depth_model)
        .with_context(|| format!("load depth: {}", depth_model.display()))?;
    let mut senses = Senses::new(detect);
    let min_confidence = config.policy().admission.min_confidence;

    let mailbox = Arc::new(Mailbox::<Sample>::default());
    let (events_tx, events_rx) = mpsc::channel();
    let shutdown = tokio_util::sync::CancellationToken::new();
    let stream = ingest::spawn(
        config.ingest(),
        Arc::clone(&mailbox),
        events_tx,
        shutdown.clone(),
    );
    let interval = Duration::from_millis(config.stream.sample_interval_ms);
    let seconds = args.seconds;
    let (empty_until, occupied_from) = (args.empty_until, args.occupied_from);
    let mut jsonl = args.jsonl.map(std::fs::File::create).transpose()?;
    eprintln!(
        "Depth meter ready; {seconds} s; model {}",
        depth_model.display()
    );
    // The models are not Send: the loop runs on this thread, outside the runtime.
    let (spans, scale, times) = tokio::task::block_in_place({
        let mailbox = Arc::clone(&mailbox);
        || -> Result<_> {
            let mut source = RtspSource::new(mailbox, events_rx, interval);
            let deadline = Instant::now() + Duration::from_secs(seconds);
            let mut spans: BTreeMap<&'static str, Span> = BTreeMap::new();
            let mut scale = Vec::new();
            let mut times = Vec::new();
            let mut origin: Option<f64> = None;
            while Instant::now() < deadline {
                let Arrival::Sample {
                    at,
                    frame: Ok(Some(frame)),
                    ..
                } = source.next(Duration::from_secs(1))
                else {
                    continue;
                };
                let t = at.source_secs.0 - *origin.get_or_insert(at.source_secs.0);
                let request = Probe::Detect { region: None };
                let Ok(Reading::Bodies(bodies)) = senses.look(&frame, &request).0 else {
                    continue;
                };
                let in_bed: Vec<_> = bodies
                    .iter()
                    .filter(|d| {
                        d.label == Label::Person
                            && d.confidence >= min_confidence
                            && site.bed.contains(d.bbox.centroid())
                    })
                    .collect();
                let started = Instant::now();
                let map = depth.estimate(&frame).context("depth")?;
                times.push(started.elapsed().as_secs_f64() * 1000.0);
                let size = (frame.width() as f32, frame.height() as f32);
                let mut finite: Vec<f64> = map
                    .values
                    .iter()
                    .filter(|v| v.is_finite())
                    .map(|v| f64::from(*v))
                    .collect();
                finite.sort_by(f64::total_cmp);
                if let (Some(lo), Some(hi)) = (finite.first(), finite.last()) {
                    scale.push((*lo, finite[finite.len() / 2], *hi));
                }
                let values: Vec<Option<f64>> = points
                    .iter()
                    .map(|p| map.sample(size, *p).map(f64::from))
                    .collect();
                let body = in_bed
                    .first()
                    .and_then(|d| map.sample(size, d.bbox.centroid()))
                    .map(f64::from);
                let span = if t <= empty_until {
                    "empty"
                } else if t < occupied_from {
                    "transition"
                } else if in_bed.is_empty() {
                    "occupied, unseen"
                } else {
                    "occupied, seen"
                };
                let entry = spans.entry(span).or_default();
                let valid: Vec<f64> = values.iter().flatten().copied().collect();
                for (i, v) in values.iter().enumerate() {
                    if let Some(v) = v {
                        entry.points[i].push(*v);
                    }
                }
                if let Some(m) = median(&valid) {
                    entry.points[5].push(m);
                }
                entry.body.extend(body);
                if let Some(file) = jsonl.as_mut() {
                    writeln!(
                        file,
                        "{}",
                        serde_json::json!({"t": t, "span": span, "in_bed": in_bed.len(), "points": values, "body": body})
                    )?;
                }
                eprintln!(
                    "t={t:6.1} {span:18} in_bed={} points={:?}",
                    in_bed.len(),
                    values
                        .iter()
                        .map(|v| v.map(|v| (v * 1000.0).round() / 1000.0))
                        .collect::<Vec<_>>()
                );
            }
            Ok((spans, scale, times))
        }
    })?;
    shutdown.cancel();
    mailbox.close();
    let _ = tokio::task::spawn_blocking(move || stream.join()).await;
    println!("{}", report(&spans, &points, &scale, &times));
    Ok(())
}
