use anyhow::Result;
use assert_cmd::prelude::*;
use assert_fs::fixture::PathChild;
use assert_fs::prelude::*;
use predicates::prelude::*;
use rexpect::ReadUntil;
use std::process::Command;

#[test]
fn initialized_the_project_with_new_dir() -> Result<()> {
    let temp_dir = assert_fs::TempDir::new()?;
    let bin_path = assert_cmd::cargo::cargo_bin("cli");

    println!("bin_path: {:?}", bin_path);
    println!("temp_dir: {:?}", temp_dir.path());
    let mut cmd = Command::new(bin_path);

    cmd.current_dir(temp_dir.path()).arg("init");

    let mut process = rexpect::session::spawn_command(cmd, Some(2000))?;
    
    process.exp_string("Project name:")?;
    process.send_line("test-project\r")?;
    process.exp_eof()?;

    temp_dir
        .child("test-project/package.json")
        .assert(predicate::path::exists())
        .assert(predicates::str::contains("\"name\": \"test-project\""));

    Ok(())
}
