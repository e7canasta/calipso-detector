use detector::config::Config;
use std::{fs, process::Command};

#[test]
fn check_prints_the_profile_and_opens_no_models() {
    for text in [
        include_str!("../../../../config/example.toml"),
        include_str!("../../../../config/test.toml"),
    ] {
        let file = std::env::temp_dir().join(format!("calipso-check-{}.toml", ulid::Ulid::new()));
        fs::write(&file, text).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_calipso-detector"))
            .args(["--check", "--config"])
            .arg(&file)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let profile: scene_contract::Profile = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(profile, Config::parse(text).unwrap().profile().unwrap());
        fs::write(
            &file,
            text.lines()
                .filter(|l| !l.starts_with("resolution ="))
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_calipso-detector"))
            .args(["--check", "--config"])
            .arg(&file)
            .output()
            .unwrap();
        fs::remove_file(&file).unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("resolution"));
    }
}
