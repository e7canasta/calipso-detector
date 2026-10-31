use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn profile(role: &str) -> String {
    let options = match role {
        "reinforce" => "bed_padding = 0.15",
        "pose" => "move_threshold = 0.1",
        // A depth nobody asks for does not pass validation: calibration asks for it.
        "depth" => "\n[calibration]\nretry_secs = 2.0",
        _ => "min_confidence = 0.30",
    };
    format!(
        "{}\n[senses.{role}]\nmodel = \"/missing/calipso-{role}.onnx\"\n{options}\n",
        include_str!("../../../../config/test.toml")
    )
}

#[test]
fn check_publishes_optional_model_without_loading_it() {
    let path = std::env::temp_dir().join(format!("calipso-startup-{}.toml", ulid::Ulid::new()));
    fs::write(&path, profile("reinforce")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_calipso-detector"))
        .args(["--check", "--config"])
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["senses"][1]["kind"], "reinforce");
}

#[test]
#[ignore = "requires CALIPSO_MODELS/yolo26s.onnx and ONNX Runtime; opens local transports"]
fn missing_reinforcement_model_terminates_at_startup_with_its_path() {
    missing_model("reinforce", "refuerzo");
}

#[test]
#[ignore = "requires CALIPSO_MODELS/yolo26s.onnx and ONNX Runtime; opens local transports"]
fn missing_faces_model_terminates_at_startup_with_its_path() {
    missing_model("faces", "caras");
}

#[test]
#[ignore = "requires CALIPSO_MODELS/yolo26s.onnx and ONNX Runtime; opens local transports"]
fn missing_pose_model_terminates_at_startup_with_its_path() {
    missing_model("pose", "pose");
}

#[test]
#[ignore = "requires CALIPSO_MODELS/yolo26s.onnx and ONNX Runtime; opens local transports"]
fn missing_depth_model_terminates_at_startup_with_its_path() {
    missing_model("depth", "profundidad");
}

fn missing_model(role: &str, name: &str) {
    assert!(
        std::env::var_os("CALIPSO_MODELS").is_some(),
        "set CALIPSO_MODELS"
    );
    let dir = std::env::temp_dir().join(format!("calipso-startup-{}", ulid::Ulid::new()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("profile.toml");
    let log = dir.join("stderr.log");
    fs::write(&path, profile(role)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_calipso-detector"))
        .arg("--config")
        .arg(&path)
        .stdout(Stdio::null())
        .stderr(fs::File::create(&log).unwrap())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        thread::sleep(Duration::from_millis(20));
    };
    let stderr = fs::read_to_string(&log).unwrap();
    fs::remove_dir_all(dir).unwrap();
    assert!(
        status.is_some(),
        "configured {role} was not loaded at startup: {stderr}"
    );
    assert!(
        !status.unwrap().success(),
        "missing model must fail startup"
    );
    assert!(
        stderr.contains(&format!("cargar {name} ONNX"))
            && stderr.contains(&format!("/missing/calipso-{role}.onnx")),
        "{stderr}"
    );
}
