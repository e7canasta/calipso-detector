use ingest::FrameDecoder;

// ffmpeg -f lavfi -i 'testsrc=size=160x96:rate=10:duration=2' \
//   -c:v libx264 -pix_fmt yuv420p -preset ultrafast -tune zerolatency \
//   -g 10 -keyint_min 10 -sc_threshold 0 -x264-params repeat-headers=1:aud=1 \
//   -f h264 tests/fixtures/testsrc.h264
const VIDEO: &[u8] = include_bytes!("fixtures/testsrc.h264");

// Each access unit begins with an AUD (NAL type 9); decode the first IDR
// together with its SPS/PPS, as Retina delivers a self-contained keyframe.
fn first_access_unit() -> &'static [u8] {
    let second_aud = VIDEO
        .windows(5)
        .enumerate()
        .filter(|(_, bytes)| *bytes == [0, 0, 0, 1, 9])
        .nth(1)
        .expect("fixture has multiple access units")
        .0;
    &VIDEO[..second_aud]
}

#[test]
fn keyframe_decodes_to_rgb_with_source_time() {
    assert!(VIDEO.len() < 50_000);
    let mut decoder = FrameDecoder::h264(90_000).unwrap();
    // Repeat after drain/flush: independent GOPs must not need intervening frames.
    for pts in [90_000, 180_000] {
        let picture = decoder
            .decode_keyframe(first_access_unit(), pts)
            .unwrap()
            .unwrap();
        assert_eq!((picture.width(), picture.height()), (160, 96));
        assert_eq!(picture.image().as_raw().len(), 160 * 96 * 3);
        assert_eq!(picture.source_time_secs, Some(pts as f64 / 90_000.0));
        let image = picture.into_image();
        assert_eq!(image.dimensions(), (160, 96));
        assert!(image.pixels().any(|p| p[0] != p[1]));
    }
}
