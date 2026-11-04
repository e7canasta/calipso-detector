//! Steps of the frame turned as it arrives (`features/shell/rotation.feature`):
//! the profile's `[stream] rotate` turns what the source hands the driver.
use crate::world::Shell;
use cucumber::{given, then, when};
use detector::{Arrival, SampleSource, Turned};
use image::{DynamicImage, Rgb, RgbImage};
use scene_contract::{SampleRef, SessionId, SourceSecs};
use std::time::{Duration, Instant};

const PROFILE: &str = include_str!("../support/profile.toml");
const STREAM: &str = "url = \"rtsp://127.0.0.1:18554/test\"";
const MARK: Rgb<u8> = Rgb([255, 0, 0]);

/// One decoded frame, then the end: the source as the driver sees it.
struct Once(Option<DynamicImage>);

impl SampleSource for Once {
    type Frame = DynamicImage;
    fn next(&mut self, _: Duration) -> Arrival<DynamicImage> {
        let Some(frame) = self.0.take() else {
            return Arrival::End;
        };
        let now = Instant::now();
        Arrival::Sample {
            at: SampleRef {
                session: SessionId(1),
                sequence: 1,
                source_secs: SourceSecs(0.0),
            },
            received: now,
            started: now,
            frame: Ok(Some(frame)),
        }
    }
}

/// The pixel a corner names, in a frame of `width` by `height`.
fn corner(name: &str, width: u32, height: u32) -> (u32, u32) {
    match name {
        "top-left" => (0, 0),
        "top-right" => (width - 1, 0),
        "bottom-right" => (width - 1, height - 1),
        "bottom-left" => (0, height - 1),
        other => panic!("no corner {other}"),
    }
}

#[given("a profile whose stream does not turn")]
fn does_not_turn(w: &mut Shell) {
    w.parse_config("");
}

/// The line goes into the profile's `[stream]` section, after its url.
#[given(expr = "a profile whose stream has {string}")]
fn stream_has(w: &mut Shell, line: String) {
    assert!(
        PROFILE.contains(STREAM),
        "the fixture's stream holds its url"
    );
    let text = PROFILE.replacen(STREAM, &format!("{STREAM}\n{line}"), 1);
    w.config = Some(detector::Config::parse(&text).map_err(|e| format!("{e:#}")));
}

#[when(regex = r"^a (\d+) by (\d+) frame marked at its ([a-z-]+) corner arrives$")]
fn frame_arrives(w: &mut Shell, width: u32, height: u32, at: String) {
    let mut frame = RgbImage::new(width, height);
    let (x, y) = corner(&at, width, height);
    frame.put_pixel(x, y, MARK);
    let rotation = w.config().stream.rotate;
    let mut source = Turned::new(Once(Some(DynamicImage::ImageRgb8(frame))), rotation);
    let Arrival::Sample { frame, .. } = source.next(Duration::ZERO) else {
        panic!("the source hands a sample");
    };
    w.image = frame.expect("the frame decodes");
}

#[then(regex = r"^the driver receives a (\d+) by (\d+) frame marked at its ([a-z-]+) corner$")]
fn driver_receives(w: &mut Shell, width: u32, height: u32, at: String) {
    let frame = w.image.as_ref().expect("a frame arrived").to_rgb8();
    assert_eq!(frame.dimensions(), (width, height), "the frame's size");
    let (x, y) = corner(&at, width, height);
    let marked: Vec<_> = frame
        .enumerate_pixels()
        .filter(|(_, _, pixel)| **pixel == MARK)
        .map(|(px, py, _)| (px, py))
        .collect();
    assert_eq!(marked, vec![(x, y)], "the mark is at the {at} corner");
}

#[then(expr = "the profile's hash is {string}")]
fn hash_is(w: &mut Shell, hash: String) {
    assert_eq!(
        w.config().profile().expect("the profile builds").hash.0,
        hash
    );
}

#[then(expr = "the profile's hash is not {string}")]
fn hash_is_not(w: &mut Shell, hash: String) {
    assert_ne!(
        w.config().profile().expect("the profile builds").hash.0,
        hash
    );
}
