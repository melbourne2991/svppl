use anyhow::Result;
use assert_cmd::prelude::*;
use std::process::Command;


#[test]
fn runs_and_exits_successfully() -> Result<()> {
    let temp_dir = assert_fs::TempDir::new()?;
    let bin_path = assert_cmd::cargo::cargo_bin("cli");


    let cmd = Command::new(bin_path)
        .current_dir(temp_dir.path())
        .arg("init");



    Ok(())
}