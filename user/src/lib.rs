use log::info;
use serde_json::Value;
use std::error::Error;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

const USER_SPACE_DIR_PATH: &str = r"user_space";

pub fn build_user_space(output_dir: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    info!("userspace dir {:?}", crate_root);
    let mut child = Command::new("cargo")
        .current_dir(crate_root.join(USER_SPACE_DIR_PATH))
        .arg("build")
        .arg("--release")
        .arg("--message-format=json-render-diagnostics")
        .stderr(Stdio::inherit())
        .stdout(Stdio::piped())
        .spawn()?;

    let output = child.stdout.take().ok_or("should have a stdout")?;
    let reader = BufReader::new(output);
    let mut artifacts = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            if v["reason"] == "compiler-artifact" {
                if let Some(exec) = v["executable"].as_str() {
                    artifacts.push(exec.to_string());
                }
            }
        }
    }
    for artifact in artifacts {
        let path = Path::new(&artifact);
        if let Some(name) = path.file_name()
            && let Some(name) = name.to_str()
        {
            let dir = output_dir.as_ref();
            let full_path = dir.join(name);
            if let Err(e) = std::fs::copy(&artifact, &full_path) {
                info!("copy file error {}", e);
                continue;
            }
            info!("file: {} done", name);
        } else {
            info!("skipping artifact {}", artifact);
        }
    }
    Ok(())
}
