use assert_cmd::Command;
use passwords::PasswordGenerator;
use predicates::prelude::predicate;
use regex::Regex;
use test_case::test_case;

#[test]
fn run_help() {
    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd.args(["--help"]).assert();

    assert.success().code(0);
}

#[test]
fn run_help_without_arguments() {
    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd.assert();

    assert.failure().code(2);
}

#[test_case("../test_config.json" ; "syspass-v3")]
#[test_case("../test_config.json" ; "syspass-v2")]
#[ignore]
fn run_search_with_no_results(version: &str) {
    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd
        .args([
            "-q",
            "-d",
            "-c",
            version,
            "search",
            "search_for_non_existent_account",
        ])
        .assert();

    assert
        .stdout(predicate::str::contains("\"jsonrpc\": String(\"2.0\"),"))
        .stdout(predicate::str::contains("Sending request to "))
        .stdout(predicate::str::contains("Received response:"))
        .failure()
        .code(1);
}

#[test_case("../test_config.json" ; "syspass-v3")]
#[test_case("../test_config.json" ; "syspass-v2")]
#[ignore]
fn run_search_with_results(version: &str) {
    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd
        .args([
            "-q", "-d", "-c", version, "search", "-i", "2", "-u", "-s", "-p",
        ])
        .assert();

    assert
        .stdout(predicate::str::contains("\"jsonrpc\": String(\"2.0\"),"))
        .stdout(predicate::str::contains("Sending request to "))
        .stdout(predicate::str::contains("Received response:"))
        .success()
        .code(0);
}

#[test_case("../test_config.json", true ; "syspass-v3")]
#[test_case("../test_config_v2.json", false ; "syspass-v2")]
#[ignore]
fn run_change_with_data(version: &str, success: bool) {
    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd
        .args([
            "-c",
            version,
            "edit",
            "password",
            "--id",
            "1",
            "--password",
            "1234",
            "-e",
            "2024-07-05",
        ])
        .assert();

    if success {
        assert.success().code(0);
    } else {
        assert.failure().code(1);
    }
}

#[test_case("../test_config.json", "category", true ; "category syspass-v3")]
#[test_case("../test_config_v2.json", "category", false ; "category syspass-v2")]
#[test_case("../test_config.json", "client", true ; "client syspass-v3")]
#[test_case("../test_config_v2.json", "client", false ; "client syspass-v2")]
#[ignore]
fn run_edit_category_client(version: &str, category_client: &str, success: bool) {
    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd
        .args([
            "-q",
            "-d",
            "-c",
            version,
            "edit",
            category_client,
            "--id",
            "1",
            "-n",
            "test_name",
            "-e",
            "test_notes",
        ])
        .assert();

    if success {
        assert.success().code(0);
    } else {
        assert.failure().code(1);
    }
}

#[test_case("../test_config.json", "category", true ; "category syspass-v3")]
#[test_case("../test_config_v2.json", "category", true ; "category syspass-v2")]
#[test_case("../test_config.json", "client", true ; "client syspass-v3")]
#[test_case("../test_config_v2.json", "client", true ; "client syspass-v2")]
#[ignore]
fn run_create_delete_category_client(version: &str, category_client: &str, success: bool) {
    let id = create_new_category_client(version, category_client).to_string();

    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd
        .args([
            "-q",
            "-d",
            "-c",
            version,
            "remove",
            category_client,
            "--id",
            id.as_str(),
        ])
        .assert();

    if success {
        assert.success().code(0);
    } else {
        assert.failure().code(1);
    }
}

fn create_new_category_client(version: &str, category_client: &str) -> u32 {
    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();

    let generator = PasswordGenerator::new()
        .length(8)
        .symbols(false)
        .spaces(false)
        .exclude_similar_characters(true)
        .strict(true)
        .numbers(true)
        .lowercase_letters(true)
        .uppercase_letters(true);

    let password = generator.generate_one().unwrap();

    let assert = cmd
        .args([
            "-q",
            "-d",
            "-c",
            version,
            "new",
            category_client,
            "-n",
            password.as_str(),
            "-e",
            "nothing",
        ])
        .assert();

    let success = assert.success();
    let reg = Regex::new(r" (:?Client|Category) .+? \((\d+)\) saved!\n$").unwrap();
    let data: String = String::from_utf8(success.get_output().clone().stdout).unwrap();

    success.code(0);

    match reg.captures(data.as_str()) {
        Some(c) => {
            let id = c[2].parse::<u32>().unwrap();
            assert!(id > 1);

            id
        }
        _ => {
            panic!("Could not create new {}", category_client);
        }
    }
}

#[test_case("../test_config.json", true ; "syspass-v3")]
#[test_case("../test_config_v2.json", true ; "syspass-v2")]
#[ignore]
fn run_new_delete_password(version: &str, success: bool) {
    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd
        .args([
            "-q",
            "-d",
            "-c",
            version,
            "new",
            "password",
            "--name",
            "new_password_test",
            "-l",
            "test",
            "-i",
            "1",
            "-a",
            "1",
            "-g",
            "0",
            "-o",
            "test",
            "-p",
            "password",
        ])
        .assert();

    let status = if success {
        assert.success().code(0)
    } else {
        assert.failure().code(1)
    };

    let reg = Regex::new(r" Account new_password_test \((\d+)\) saved!\n$").unwrap();
    let data: String = String::from_utf8(status.get_output().clone().stdout).unwrap();

    let id = match reg.captures(data.as_str()) {
        Some(c) => {
            let id = c[1].parse::<u32>().unwrap();
            assert!(id > 1);

            id
        }
        _ => {
            panic!("Could not create new account");
        }
    }
    .to_string();

    let mut cmd = Command::cargo_bin("syspass-cli").unwrap();
    let assert = cmd
        .args([
            "-q",
            "-d",
            "-c",
            version,
            "remove",
            "account",
            "-i",
            id.as_str(),
        ])
        .assert();

    assert.success().code(0);
}
