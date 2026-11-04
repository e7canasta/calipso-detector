use super::*;
use scene_core::policy::{Band, Burst, ShadowPolicy, TwoPlanesPolicy};
use scene_core::units::{FeetMargin, Metres, PositiveMetres, Ratio, RelativeDepth};

/// Every profile in the repository loads and builds what the detector
/// builds when it starts: the policy, the site map, the ingest and the
/// published profile. The kernel builds every defconfig; this is ours.
#[test]
fn every_profile_in_the_repository_loads() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config");
    let mut profiles: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "toml"))
        .collect();
    profiles.sort();
    assert!(!profiles.is_empty(), "no profile in {}", dir.display());
    for path in profiles {
        let config = Config::load(&path).unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
        config
            .profile()
            .unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
        let _ = (
            config.policy().expect("a valid profile"),
            config.site_map(),
            config.ingest(),
            config.prefix(),
        );
    }
}
pub(super) const VALID: &str = include_str!("../../tests/support/profile.toml");

/// `text` is refused, and the error names `field`.
fn refused(text: &str, field: &str) {
    let error = Config::parse(text).unwrap_err();
    assert!(error.to_string().contains(field), "{error}");
}

/// Each `(from, to, field)` case is applied to `section`, which follows the
/// valid profile: the result is refused, and the error names `field`.
fn refused_section(section: &str, cases: &[(&str, &str, &str)]) {
    for (from, to, field) in cases {
        refused(&format!("{VALID}{}", section.replace(from, to)), field);
    }
}

/// Each `(from, to, field)` case is applied to the whole `text`: the result is
/// refused, and the error names `field`.
fn refused_text(text: &str, cases: &[(&str, &str, &str)]) {
    for (from, to, field) in cases {
        refused(&text.replace(from, to), field);
    }
}
/// The verifier: the policy maps one to one; the sense and the policy come
/// together; the bands are checked; a profile without it keeps its hash.
#[test]
fn verify_section_becomes_the_policy_and_is_checked() {
    let base = format!(
        "{VALID}\n[senses.depth]\nmodel = \"yolo26s-depth.onnx\"\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n"
    );
    let map = "headboard = [100, 100, 500, 170]\n";
    let with_headboard = base.replacen("[map]\n", &format!("[map]\n{map}"), 1);
    let posture = "\n[bed_posture]\n";
    let sense = "\n[senses.verify]\n";
    let verify = "\n[verify]\nmax_per_hour = 20\ntimeout_secs = 8\ndoubtful_disparity = [0.03, 0.12]\ndoubtful_feet_margin = 0.06\n[verify.held]\nafter_secs = 30\nvalid_secs = 3600\n[verify.frames]\nsitting_in_bed = { frames = 3, every_secs = 1.5 }\n[verify.sharpness]\nmin_ratio = 0.5\nmax_wait_secs = 4\n";
    let text = format!("{with_headboard}{sense}{posture}{verify}");
    let config = Config::parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    let policy = config.policy().expect("a valid profile").verify.unwrap();
    assert_eq!(policy.max_per_hour, Some(20));
    assert_eq!(policy.timeout_secs.map(|s| s.get()), Some(8.0));
    assert_eq!(
        policy.doubtful_disparity,
        Some(Band {
            lo: RelativeDepth::new(0.03).expect("a threshold"),
            hi: RelativeDepth::new(0.12).expect("a threshold"),
        })
    );
    assert_eq!(
        policy.doubtful_feet_margin,
        Some(FeetMargin::new(0.06).expect("a margin"))
    );
    assert_eq!(
        (
            policy.frames.sitting.frames,
            policy.frames.sitting.every_secs.get()
        ),
        (3, 1.5)
    );
    assert_eq!(policy.frames.held, Burst::ONE);
    let sharp = policy.sharpness.unwrap();
    assert_eq!(
        (
            sharp.min_ratio.get(),
            sharp.window,
            sharp.max_wait_secs.get()
        ),
        (0.5, 20, 4.0)
    );
    let held = policy.held.unwrap();
    assert_eq!(
        (held.after_secs.get(), held.valid_secs.map(|s| s.get())),
        (30.0, Some(3600.0))
    );
    assert_eq!(
        config.senses.verify.as_ref().unwrap().subject,
        "calipso.verify.v1"
    );
    assert!(
        Config::parse(VALID)
            .unwrap()
            .policy()
            .expect("a valid profile")
            .verify
            .is_none()
    );
    for (bad, field) in [
        (
            format!("{with_headboard}{posture}{verify}"),
            "senses.verify",
        ),
        (
            format!("{with_headboard}{sense}"),
            "senses.verify without [verify]",
        ),
        (
            format!(
                "{with_headboard}{sense}{posture}{}",
                verify.replace("[0.03, 0.12]", "[0.12, 0.03]")
            ),
            "doubtful_disparity",
        ),
        (format!("{with_headboard}{sense}{verify}"), "bed_posture"),
        (
            format!(
                "{with_headboard}{sense}{posture}{}",
                verify.replace("0.06", "0.7")
            ),
            "doubtful_feet_margin",
        ),
        (
            format!(
                "{with_headboard}{sense}{posture}{}",
                verify.replace("max_per_hour = 20", "max_per_hour = 0")
            ),
            "max_per_hour",
        ),
    ] {
        let error = Config::parse(&bad).unwrap_err();
        assert!(error.to_string().contains(field), "{field}: {error}");
    }
    let hashed = Config::parse(&text).unwrap().profile().unwrap().hash;
    let without = format!("{with_headboard}{posture}");
    assert_ne!(
        hashed,
        Config::parse(&without).unwrap().profile().unwrap().hash
    );
}

#[test]
fn reinforce_section_enables_policy_and_rejects_invalid_parameters() {
    let section = "\n[senses.reinforce]\nmodel = \"yolo26l.onnx\"\nbackend_confidence = 0.25\nbed_padding = 0.15\n";
    let config = Config::parse(&format!("{VALID}{section}")).unwrap();
    assert_eq!(
        config
            .policy()
            .expect("a valid profile")
            .reinforce
            .unwrap()
            .bed_padding
            .get(),
        0.15
    );
    assert!(
        Config::parse(VALID)
            .unwrap()
            .policy()
            .expect("a valid profile")
            .reinforce
            .is_none()
    );
    refused_section(
        section,
        &[
            ("yolo26l.onnx", "", "model"),
            ("0.25", "-0.1", "backend_confidence"),
            ("0.25", "1.1", "backend_confidence"),
            ("0.25", "nan", "backend_confidence"),
            ("0.25", "inf", "backend_confidence"),
            ("0.25", "0.60", "backend_confidence"),
            ("0.15", "-0.1", "bed_padding"),
            ("0.15", "nan", "bed_padding"),
            ("0.15", "inf", "bed_padding"),
        ],
    );
    assert!(Config::parse(&format!("{VALID}{section}unknown = 1\n")).is_err());
}

#[test]
fn reinforcement_preserves_the_baseline_hash_and_comes_after_detection() {
    use scene_contract::LookKind;
    let legacy = Config::parse(VALID).unwrap().profile().unwrap();
    assert_eq!(legacy.hash.0, "sha256:239a5803f0c15901");
    let text =
        format!("{VALID}\n[senses.reinforce]\nmodel = \"yolo26l.onnx\"\nbed_padding = 0.15\n");
    let profile = Config::parse(&text).unwrap().profile().unwrap();
    assert_ne!(profile.hash, legacy.hash);
    assert_eq!(
        profile.senses.iter().map(|s| s.kind).collect::<Vec<_>>(),
        vec![LookKind::Detect, LookKind::Reinforce]
    );
    assert_eq!(profile.senses[1].model, "yolo26l.onnx");
    assert_eq!(profile.senses[1].backend_confidence, 0.25);
    assert_eq!(profile.senses[1].min_confidence, Some(0.5));
    let changed_padding = Config::parse(&text.replace("0.15", "0.20"))
        .unwrap()
        .profile()
        .unwrap();
    assert_ne!(profile.hash, changed_padding.hash);
}

#[test]
fn model_paths_resolve_relative_and_absolute_paths_for_every_role() {
    for model in ["yolo26s.onnx", "yolo26l.onnx", "face/yolov12l-face.onnx"] {
        let model = Path::new(model);
        assert_eq!(
            resolve_model_path(model, Some(Path::new("/models"))),
            Path::new("/models").join(model)
        );
        assert_eq!(resolve_model_path(model, None), model);
    }
    let absolute = Path::new("/external/yolo26l.onnx");
    assert_eq!(
        resolve_model_path(absolute, Some(Path::new("/models"))),
        absolute
    );
    assert_eq!(Config::parse(VALID).unwrap().model_path(absolute), absolute);
}

#[test]
fn faces_section_enables_policy_names_roles_and_preserves_legacy_hash() {
    use scene_contract::LookKind;
    let baseline = Config::parse(VALID).unwrap();
    assert!(baseline.policy().expect("a valid profile").faces.is_none());
    assert_eq!(
        baseline.profile().unwrap().hash.0,
        "sha256:239a5803f0c15901"
    );
    let section = "\n[senses.faces]\nmodel = \"face/yolov12l-face.onnx\"\nbackend_confidence = 0.25\nmin_confidence = 0.30\n";
    let config = Config::parse(&format!("{VALID}{section}")).unwrap();
    assert_eq!(
        config
            .policy()
            .expect("a valid profile")
            .faces
            .unwrap()
            .min_confidence
            .get(),
        0.30
    );
    let profile = config.profile().unwrap();
    assert_ne!(profile.hash, baseline.profile().unwrap().hash);
    assert_eq!(
        profile.senses.iter().map(|s| s.kind).collect::<Vec<_>>(),
        vec![LookKind::Detect, LookKind::Faces]
    );
    assert_eq!(profile.senses[1].min_confidence, Some(0.30));
    let both = Config::parse(&format!(
        "{VALID}{section}\n[senses.reinforce]\nmodel = \"yolo26l.onnx\"\nbed_padding = 0.15\n"
    ))
    .unwrap()
    .profile()
    .unwrap();
    assert_eq!(
        both.senses.iter().map(|s| s.kind).collect::<Vec<_>>(),
        vec![LookKind::Detect, LookKind::Reinforce, LookKind::Faces]
    );
    refused_section(
        section,
        &[
            ("face/yolov12l-face.onnx", "", "model"),
            ("0.25", "-0.1", "backend_confidence"),
            ("0.25", "1.1", "backend_confidence"),
            ("0.25", "nan", "backend_confidence"),
            ("0.25", "inf", "backend_confidence"),
            ("0.25", "0.40", "backend_confidence"),
            ("0.30", "-0.1", "min_confidence"),
            ("0.30", "1.1", "min_confidence"),
            ("0.30", "nan", "min_confidence"),
            ("0.30", "inf", "min_confidence"),
        ],
    );
    assert!(Config::parse(&format!("{VALID}{section}unknown = 1\n")).is_err());
}

#[test]
fn pose_section_enables_enrich_validates_and_preserves_legacy_hash() {
    use scene_contract::LookKind;
    let baseline = Config::parse(VALID).unwrap();
    assert!(baseline.policy().expect("a valid profile").enrich.is_none());
    let section = "\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\nbackend_confidence = 0.25\nmove_threshold = 0.1\nmin_keypoint_score = 0.3\n";
    let config = Config::parse(&format!("{VALID}{section}")).unwrap();
    let enrich = config.policy().expect("a valid profile").enrich.unwrap();
    assert_eq!(
        (
            enrich.move_threshold.get(),
            enrich.depth,
            enrich.min_keypoint_score.get()
        ),
        (0.1, false, 0.3)
    );
    let defaults = Config::parse(&format!(
        "{VALID}\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n"
    ))
    .unwrap();
    assert_eq!(
        defaults.policy().expect("a valid profile").enrich,
        config.policy().expect("a valid profile").enrich
    );
    assert_eq!(
        defaults.profile().unwrap().hash,
        config.profile().unwrap().hash
    );
    let profile = config.profile().unwrap();
    assert_ne!(profile.hash.0, "sha256:239a5803f0c15901");
    assert_eq!(profile.senses[1].kind, LookKind::Pose);
    assert_eq!(profile.senses[1].model, "yolo26s-pose.onnx");
    assert_eq!(profile.senses[1].min_confidence, None);
    refused_section(
        section,
        &[
            ("yolo26s-pose.onnx", "", "model"),
            ("0.25", "1.1", "backend_confidence"),
            ("0.25", "nan", "backend_confidence"),
            ("0.1\n", "-0.1\n", "move_threshold"),
            ("0.1\n", "inf\n", "move_threshold"),
            ("0.3\n", "-0.1\n", "min_keypoint_score"),
            ("0.3\n", "1.5\n", "min_keypoint_score"),
        ],
    );
    assert!(Config::parse(&format!("{VALID}{section}unknown = 1\n")).is_err());
}

#[test]
fn the_shadow_needs_the_bed_size_validates_and_leaves_the_hash_when_absent() {
    let baseline = Config::parse(VALID).unwrap();
    assert!(baseline.policy().expect("a valid profile").shadow.is_none());
    assert!(baseline.site_map().bed_floor.is_none());
    let map =
        "bed_floor = [[110, 150], [490, 150], [490, 430], [110, 430]]\nbed_size_m = [0.9, 2.0]\n";
    let with_map = VALID.replacen("[map]\n", &format!("[map]\n{map}"), 1);
    let shadow = "\n[shadow]\nnear_m = 0.6\n";
    let config = Config::parse(&format!("{with_map}{shadow}")).unwrap();
    assert_eq!(
        config.policy().expect("a valid profile").shadow,
        Some(ShadowPolicy {
            near_m: PositiveMetres::new(0.6).expect("a distance"),
            support_margin_m: Metres::new(0.0).expect("a distance"),
        })
    );
    let site = config.site_map();
    assert_eq!(site.bed_size_m, Some([0.9, 2.0]));
    assert!(site.bed_floor.is_some());
    // The map's keys alone change no policy and keep their own hash.
    let map_only = Config::parse(&with_map).unwrap();
    assert!(map_only.policy().expect("a valid profile").shadow.is_none());
    assert_ne!(
        map_only.profile().unwrap().hash,
        config.profile().unwrap().hash
    );
    refused(&format!("{VALID}{shadow}"), "bed_size_m");
    refused_text(
        &format!("{with_map}{shadow}"),
        &[
            ("near_m = 0.6", "near_m = 0", "shadow.near_m"),
            ("near_m = 0.6", "near_m = nan", "shadow.near_m"),
            ("[0.9, 2.0]", "[0.9, -2.0]", "map.bed_size_m"),
            ("[490, 430]", "[990, 430]", "map.bed_floor"),
        ],
    );
    assert!(Config::parse(&format!("{with_map}{shadow}unknown = 1\n")).is_err());
}

#[test]
fn two_planes_needs_its_geometry_validates_and_leaves_the_hash_when_absent() {
    let baseline = Config::parse(VALID).unwrap();
    assert!(
        baseline
            .policy()
            .expect("a valid profile")
            .two_planes
            .is_none()
    );
    let map =
        "bed_floor = [[110, 150], [490, 150], [490, 430], [110, 430]]\nbed_size_m = [0.9, 2.0]\n";
    let with_map = VALID.replacen("[map]\n", &format!("[map]\n{map}"), 1);
    let occupancy = "\n[occupancy]\ngeometry = \"two_planes\"\n\n[occupancy.two_planes]\nshare = 0.45\nsupport_m = 0.10\nnear_m = 0.60\n";
    let config = Config::parse(&format!("{with_map}{occupancy}")).unwrap();
    assert_eq!(
        config.policy().expect("a valid profile").two_planes,
        Some(TwoPlanesPolicy {
            share: Ratio::new(0.45).expect("a share"),
            support_m: Metres::new(0.10).expect("a distance"),
            near_m: PositiveMetres::new(0.60).expect("a distance"),
            knee_bent_deg: None,
        })
    );
    // Frontal said out loud is frontal, with the hash of its own section.
    let frontal = Config::parse(&format!(
        "{with_map}\n[occupancy]\ngeometry = \"frontal\"\n"
    ))
    .unwrap();
    assert!(
        frontal
            .policy()
            .expect("a valid profile")
            .two_planes
            .is_none()
    );
    let map_only = Config::parse(&with_map).unwrap();
    assert_ne!(
        map_only.profile().unwrap().hash,
        config.profile().unwrap().hash
    );
    refused(&format!("{VALID}{occupancy}"), "map.bed_floor");
    refused_text(
        &format!("{with_map}{occupancy}"),
        &[
            ("share = 0.45", "share = 0", "two_planes.share"),
            ("share = 0.45", "share = 1.5", "two_planes.share"),
            ("support_m = 0.10", "support_m = -1", "two_planes.support_m"),
            ("near_m = 0.60", "near_m = 0", "two_planes.near_m"),
            (
                "geometry = \"two_planes\"",
                "geometry = \"frontal\"",
                "needs occupancy.geometry",
            ),
        ],
    );
}

#[test]
fn the_side_exit_is_off_unless_asked_and_leaves_the_hash_when_absent() {
    let baseline = Config::parse(VALID).unwrap();
    assert!(!baseline.policy().expect("a valid profile").held_left);
    let off = Config::parse(&format!("{VALID}\n[occupancy]\nheld_left = false\n")).unwrap();
    let on = Config::parse(&format!("{VALID}\n[occupancy]\nheld_left = true\n")).unwrap();
    assert!(!off.policy().expect("a valid profile").held_left);
    assert!(on.policy().expect("a valid profile").held_left);
    assert_ne!(off.profile().unwrap().hash, on.profile().unwrap().hash);
}

#[test]
fn near_first_is_off_unless_asked_and_leaves_the_hash_when_absent() {
    // An `[occupancy]` already in the profile: the key alone is measured.
    let occupancy = format!("{VALID}\n[occupancy]\nheld_left = true\n");
    let baseline = Config::parse(&occupancy).unwrap();
    assert!(!baseline.policy().expect("a valid profile").near_first);
    let off = Config::parse(&format!("{occupancy}near_first = false\n")).unwrap();
    let on = Config::parse(&format!("{occupancy}near_first = true\n")).unwrap();
    assert!(!off.policy().expect("a valid profile").near_first);
    assert!(on.policy().expect("a valid profile").near_first);
    assert_eq!(
        baseline.profile().unwrap().hash,
        off.profile().unwrap().hash
    );
    assert_ne!(off.profile().unwrap().hash, on.profile().unwrap().hash);
}

#[test]
fn the_knee_needs_two_planes_and_the_pose_validates_and_leaves_the_hash_when_absent() {
    let map =
        "bed_floor = [[110, 150], [490, 150], [490, 430], [110, 430]]\nbed_size_m = [0.9, 2.0]\n";
    let with_map = VALID.replacen("[map]\n", &format!("[map]\n{map}"), 1);
    let pose = "\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n";
    let occupancy = "\n[occupancy]\ngeometry = \"two_planes\"\n\n[occupancy.two_planes]\nshare = 0.45\nsupport_m = 0.10\nnear_m = 0.60\n";
    let knee = "\n[occupancy.knee]\nbent_deg = 150\n";
    let without = Config::parse(&format!("{with_map}{pose}{occupancy}")).unwrap();
    let with = Config::parse(&format!("{with_map}{pose}{occupancy}{knee}")).unwrap();
    assert_eq!(
        without
            .policy()
            .expect("a valid profile")
            .two_planes
            .unwrap()
            .knee_bent_deg,
        None
    );
    assert_eq!(
        with.policy()
            .expect("a valid profile")
            .two_planes
            .unwrap()
            .knee_bent_deg
            .map(|d| d.get()),
        Some(150.0)
    );
    assert_ne!(
        without.profile().unwrap().hash,
        with.profile().unwrap().hash
    );
    refused(&format!("{with_map}{occupancy}{knee}"), "[senses.pose]");
    refused(
        &format!("{with_map}{pose}\n[occupancy]\ngeometry = \"frontal\"\n{knee}"),
        "occupancy.knee needs occupancy.geometry",
    );
    refused(
        &format!("{with_map}{pose}{occupancy}{}", knee.replace("150", "180")),
        "occupancy.knee.bent_deg",
    );
    let sizeless = with_map.replace("bed_size_m = [0.9, 2.0]\n", "");
    let error = Config::parse(&format!("{sizeless}{occupancy}")).unwrap_err();
    assert!(error.to_string().contains("bed_size_m"), "{error}");
    let sectionless = format!("{with_map}\n[occupancy]\ngeometry = \"two_planes\"\n");
    let error = Config::parse(&sectionless).unwrap_err();
    assert!(
        error.to_string().contains("[occupancy.two_planes]"),
        "{error}"
    );
    assert!(Config::parse(&format!("{with_map}{occupancy}unknown = 1\n")).is_err());
}

#[test]
fn depth_and_calibration_enable_policies_validate_together_and_keep_legacy_hash() {
    use scene_contract::LookKind;
    let baseline = Config::parse(VALID).unwrap();
    assert_eq!(
        baseline.profile().unwrap().hash.0,
        "sha256:239a5803f0c15901"
    );
    assert!(
        baseline
            .policy()
            .expect("a valid profile")
            .calibration
            .is_none()
    );
    let pose = "\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n";
    let depth = "\n[senses.depth]\nmodel = \"yolo26s-depth.onnx\"\n";
    let calibration = "\n[calibration]\nretry_secs = 30.0\n";
    let kinds = |text: &str| {
        let config = Config::parse(&format!("{VALID}{text}")).unwrap();
        let profile = config.profile().unwrap();
        assert!(
            profile.senses[1..]
                .iter()
                .all(|s| s.min_confidence.is_none())
        );
        (
            config.policy().expect("a valid profile"),
            profile.hash,
            profile.senses.iter().map(|s| s.kind).collect::<Vec<_>>(),
        )
    };
    let (policy, with_pose, roles) = kinds(&format!("{pose}{depth}"));
    assert!(policy.enrich.unwrap().depth && policy.calibration.is_none());
    assert_eq!(roles, [LookKind::Detect, LookKind::Pose, LookKind::Depth]);
    let (policy, with_calibration, roles) = kinds(&format!("{depth}{calibration}"));
    assert!(policy.enrich.is_none());
    assert_eq!(policy.calibration.unwrap().retry_secs.get(), 30.0);
    assert_eq!(roles, [LookKind::Detect, LookKind::Calibrate]);
    let (_, all, roles) = kinds(&format!("{pose}{depth}{calibration}"));
    assert_eq!(
        roles,
        [
            LookKind::Detect,
            LookKind::Pose,
            LookKind::Depth,
            LookKind::Calibrate
        ]
    );
    let (_, other_retry, _) = kinds(&format!("{depth}\n[calibration]\nretry_secs = 60.0\n"));
    let (_, default_retry, _) = kinds(&format!("{depth}\n[calibration]\n"));
    let (_, explicit_default, _) = kinds(&format!("{depth}\n[calibration]\nretry_secs = 2.0\n"));
    assert_eq!(default_retry, explicit_default);
    for hash in [&with_pose, &with_calibration, &all, &other_retry] {
        assert_ne!(hash.0, "sha256:239a5803f0c15901");
    }
    assert_ne!(with_calibration, other_retry);
    refused(&format!("{VALID}{calibration}"), "calibration");
    refused(&format!("{VALID}{depth}"), "senses.depth");
    refused(
        &format!("{VALID}{pose}{}", depth.replace("yolo26s-depth.onnx", "")),
        "senses.depth.model",
    );
    refused(
        &format!("{VALID}{depth}\n[calibration]\nretry_secs = -1.0\n"),
        "calibration.retry_secs",
    );
    refused(
        &format!("{VALID}{depth}\n[calibration]\nretry_secs = nan\n"),
        "calibration.retry_secs",
    );
    assert!(Config::parse(&format!("{VALID}{pose}{depth}unknown = 1\n")).is_err());
    assert!(Config::parse(&format!("{VALID}{depth}{calibration}unknown = 1\n")).is_err());
}

#[test]
fn the_hash_covers_only_the_scene_and_normalizes_defaults() {
    let baseline = Config::parse(VALID).unwrap().profile().unwrap();
    assert_eq!(baseline.hash.0.len(), 23);
    let changed_transport = VALID
        .replace("room-1", "room-2")
        .replace("18554", "18555")
        .replace("4222", "4223")
        .replace(
            "[senses.detect]",
            "username = \"private\"\npassword = \"secret\"\n[senses.detect]",
        );
    assert_eq!(
        Config::parse(&changed_transport)
            .unwrap()
            .profile()
            .unwrap()
            .hash,
        baseline.hash
    );
    let explicit_defaults = format!(
        "{VALID}\n[policy]\ndwell_secs = 2.0\nmin_confidence = 0.5\nsame_body_iou = 0.5\nnear_bed_margin = 0.1\n"
    );
    assert_eq!(
        Config::parse(&explicit_defaults)
            .unwrap()
            .profile()
            .unwrap()
            .hash,
        baseline.hash
    );
    for changed in [
        VALID.replace("yolo26s", "yolo26m"),
        VALID.replace("500,400", "490,400"),
        format!("{VALID}\n[policy]\ndwell_secs = 3.0"),
    ] {
        assert_ne!(
            Config::parse(&changed).unwrap().profile().unwrap().hash,
            baseline.hash
        );
    }
    let mut c = Config::parse(VALID).unwrap();
    c.senses.detect.backend_confidence = 0.1;
    assert_ne!(c.profile().unwrap().hash, baseline.hash);
    c = Config::parse(VALID).unwrap();
    c.map.resolution = [801, 600];
    assert_ne!(c.profile().unwrap().hash, baseline.hash);
}
#[test]
fn config_validates_and_translates_without_io() {
    let c = Config::parse(VALID).unwrap();
    assert_eq!(c.site_map().resolution.unwrap().width, 800);
    assert_eq!(
        c.policy()
            .expect("a valid profile")
            .persistence
            .dwell_secs
            .get(),
        2.0
    );
    assert_eq!(c.prefix(), "vision.test.room-1");
    assert_eq!(c.ingest().sampling, ingest::Sampling::KeyframesOnly);
}
#[test]
fn resolution_is_required_with_a_reason() {
    let e = Config::parse(&VALID.replace("resolution = [800, 600]", "")).unwrap_err();
    assert!(e.to_string().contains("resolution"), "{e}");
}
#[test]
fn rejects_invalid_policies_and_v2_options() {
    for extra in [
        "dwell_secs = -1.0",
        "dwell_secs = nan",
        "min_confidence = nan",
        "same_body_iou = 2.0",
        "observations = 3",
    ] {
        assert!(
            Config::parse(&format!("{VALID}\n[policy]\n{extra}")).is_err(),
            "{extra}"
        );
    }
    assert!(Config::parse(&format!("{VALID}\n[stability]\nobservations = 3")).is_err());
    assert!(Config::parse(&VALID.replace("[800, 600]", "[0, 600]")).is_err());
}

#[test]
fn a_posture_tolerance_that_is_negative_or_not_finite_is_refused_by_name() {
    let pose = "\n[senses.pose]\nmodel = \"yolo26s-pose.onnx\"\n";
    for value in ["-0.05", "nan"] {
        let depth = format!(
            "\n[senses.depth]\nmodel = \"yolo26s-depth.onnx\"\nposture_tolerance = {value}\n"
        );
        refused(
            &format!("{VALID}{pose}{depth}"),
            "senses.depth.posture_tolerance",
        );
    }
    let depth = "\n[senses.depth]\nmodel = \"yolo26s-depth.onnx\"\nposture_tolerance = 0.05\n";
    assert!(Config::parse(&format!("{VALID}{pose}{depth}")).is_ok());
}

/// F2.3a: a profile without `[occupancy.parts]` keeps the hash it had before
/// the section existed; the section changes it.
#[test]
fn parts_section_leaves_the_hash_when_absent() {
    let text = include_str!("../../tests/support/parts.toml");
    let hash = Config::parse(text).unwrap().profile().unwrap().hash.0;
    assert_eq!(hash, "sha256:222c39c4eea73c46");
    let with = Config::parse(&format!("{text}\n[occupancy.parts]\n")).unwrap();
    assert_ne!(with.profile().unwrap().hash.0, hash);
}

/// F2.3a: the section's numbers reach `Policy::parts` in their units; an empty
/// section is the sprint 1 values; a number out of range does not load.
#[test]
fn parts_section_reaches_the_policy_in_its_units() {
    let text = include_str!("../../tests/support/parts.toml");
    let empty = Config::parse(&format!("{text}\n[occupancy.parts]\n")).unwrap();
    let parts = empty
        .policy()
        .expect("a valid profile")
        .parts
        .expect("the section");
    assert_eq!(parts.far_strip_m, Metres::new(0.18).expect("a distance"));
    assert_eq!(parts.ankle_proof_score.get(), 0.75);
    assert_eq!(parts.pelvis_band.lo().get(), 0.6);
    assert_eq!(parts.pelvis_band.hi().get(), 1.4);
    assert_eq!(parts.cut_tolerance.get(), 0.12);
    assert_eq!(parts.headboard_w.lo().get(), -0.15);
    assert_eq!(parts.headboard_w.hi().get(), 1.0);
    // Without the section the policy has none: the engine as it was.
    let without = Config::parse(text).unwrap();
    assert!(without.policy().expect("a valid profile").parts.is_none());
    for section in [
        "far_strip_m = -0.1",
        "ankle_proof_score = 1.5",
        "cut_tolerance = 2",
        "pelvis_band = [1.4, 0.6]",
        "headboard_w = [0.0, nan]",
    ] {
        assert!(
            Config::parse(&format!("{text}\n[occupancy.parts]\n{section}\n")).is_err(),
            "{section} must not load"
        );
    }
}
