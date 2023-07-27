use assert_cmd::Command;

#[test]
fn run_help() {
    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd
        .args(["--config", "test_config.json", "--help"])
        .assert();

    assert
        .success()
        .code(0);
}

#[test]
fn run_help_without_arguments() {
    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd
        .args(["--config", "test_config.json"])
        .assert();

    assert
        .failure()
        .code(2);
}

#[test]
#[ignore]
fn run_change_with_data() {
    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd
        .args(["--config", "test_config.json", "edit", "password", "--id", "1", "--password", "1234", "-e", "2024-07-05"])
        .assert();

    assert
        .success()
        .code(0);
}
