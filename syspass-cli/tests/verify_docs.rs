use assert_cmd::Command;
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize, Debug, Clone)]
struct DocParam {
    name: String,
    #[serde(rename = "type")]
    doc_type: Option<String>,
    required: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
struct DocMethod {
    params: Vec<DocParam>,
}

#[derive(Deserialize, Debug, Clone)]
struct DocTable {
    version: String,
    source: String,
    methods: BTreeMap<String, DocMethod>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    MethodNotInDocs,
    UndocumentedParam,
    MissingRequired,
    TypeMismatch,
    ParamNameDivergence,
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let name = match self {
            Self::MethodNotInDocs => "method-not-in-docs",
            Self::UndocumentedParam => "undocumented-param",
            Self::MissingRequired => "missing-required",
            Self::TypeMismatch => "type-mismatch",
            Self::ParamNameDivergence => "param-name-divergence",
        };
        write!(f, "{name}")
    }
}

#[derive(Debug)]
struct Finding {
    kind: Kind,
    param: String,
    detail: String,
}

impl Finding {
    /// Contract violations: a method the docs do not have, a param the docs require and the
    /// CLI does not send, or a documented param replaced by a differently named one. Extra
    /// params and string-vs-int shapes are warnings: the CLI sends every param as a JSON
    /// string and always attaches `tokenPass` on password paths.
    const fn is_hard(&self) -> bool {
        matches!(
            self.kind,
            Kind::MethodNotInDocs | Kind::MissingRequired | Kind::ParamNameDivergence
        )
    }
}

#[derive(Debug)]
struct SentRequest {
    method: String,
    params: BTreeMap<String, String>,
}

fn load_table(path: &str) -> DocTable {
    let text = std::fs::read_to_string(path).expect("Fixture should be readable");
    serde_json::from_str(&text).expect("Fixture should parse as JSON")
}

/// The CLI sends every param as a JSON string, so a documented int or array is checked by
/// looking at the value it carries. Secret params are masked in the debug output
/// (`src/api/syspass.rs:34`), so those are presence-only checks.
fn value_matches_type(value: &str, doc_type: &str) -> bool {
    match doc_type {
        "int" => value.parse::<i64>().is_ok(),
        "array" => value.starts_with('['),
        _ => true,
    }
}

fn check(table: &DocTable, request: &SentRequest) -> Vec<Finding> {
    let Some(doc) = table.methods.get(&request.method) else {
        return vec![Finding {
            kind: Kind::MethodNotInDocs,
            param: String::new(),
            detail: format!("{} is not in the documented table", request.method),
        }];
    };

    let mut findings = Vec::new();
    let mut extra = Vec::new();
    let mut absent = Vec::new();

    for (name, value) in &request.params {
        match doc.params.iter().find(|p| &p.name == name) {
            Some(param) => {
                if let Some(doc_type) = &param.doc_type
                    && !value_matches_type(value, doc_type)
                {
                    findings.push(Finding {
                        kind: Kind::TypeMismatch,
                        param: name.clone(),
                        detail: format!("documented as {doc_type}, sent as {value:?}"),
                    });
                }
            }
            None => extra.push(name.clone()),
        }
    }

    for param in &doc.params {
        if request.params.contains_key(&param.name) {
            continue;
        }

        absent.push(param.name.clone());

        if param.required == Some(true) {
            findings.push(Finding {
                kind: Kind::MissingRequired,
                param: param.name.clone(),
                detail: "required by the docs but not sent".to_string(),
            });
        }
    }

    // An extra param that the docs name for another method is a rename of one this method
    // lost, not an addition. A param the docs never name anywhere is a plain warning.
    let renames: Vec<String> = extra
        .iter()
        .filter(|name| table.methods.values().any(|m| m.params.iter().any(|p| &p.name == *name)))
        .cloned()
        .collect();

    if !absent.is_empty() && !renames.is_empty() {
        findings.push(Finding {
            kind: Kind::ParamNameDivergence,
            param: format!("{} vs {}", absent.join(","), renames.join(",")),
            detail: "documented param is absent while another name takes its place".to_string(),
        });
    } else {
        for name in &extra {
            findings.push(Finding {
                kind: Kind::UndocumentedParam,
                param: name.clone(),
                detail: "sent but not documented for this method".to_string(),
            });
        }
    }

    findings
}

/// One entry per `Sending request to ...` block. The request is logged before the connection
/// is attempted (`src/api/syspass.rs:134`), so a dead host still shows what the CLI built.
/// A block that cannot be parsed is reported as a problem instead of being skipped, and the
/// parsed count has to match the number of blocks, otherwise a format change would quietly
/// drop requests from coverage.
fn requests_in_output(stdout: &str) -> (Vec<SentRequest>, Vec<String>) {
    let re = Regex::new(r#"method: "([^"]+)", params: (\{.*?\}), id: \d+"#).expect("Regex should compile");
    let blocks = stdout.matches("Sending request to").count();

    let mut problems = Vec::new();
    let requests = re
        .captures_iter(stdout)
        .filter_map(|c| {
            let parsed: Value = match serde_json::from_str(&c[2]) {
                Ok(v) => v,
                Err(e) => {
                    problems.push(format!("could not parse params block for {}: {e}", &c[1]));
                    return None;
                }
            };

            Some(SentRequest {
                method: c[1].to_string(),
                params: parsed
                    .as_object()?
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect::<BTreeMap<_, _>>(),
            })
        })
        .collect::<Vec<_>>();

    let parsed = requests.len();

    if parsed != blocks {
        problems.push(format!(
            "parsed {parsed} of {blocks} request blocks; the debug format may have changed"
        ));
    }

    (requests, problems)
}

fn run_cli(config: &str, command: &str) -> String {
    let output = Command::cargo_bin("syspass-cli")
        .expect("Binary should be built")
        .args(["-q", "-d", "-c", config])
        .args(command.split(' '))
        .output()
        .expect("Command should run");

    String::from_utf8_lossy(&output.stdout).to_string()
}

struct Step {
    config: &'static str,
    command: &'static str,
    expected: &'static str,
}

/// One request per command. Paths that need a server reply before the next call (a fetch
/// feeding an edit, or a search result feeding the password read) stop at the first request
/// against a dead host, so they are recorded as gaps instead of being driven here.
fn steps() -> Vec<Step> {
    [
        (V3, "search -p test-ssh", "account/search"),
        (V3, "search -i 1 -p x", "account/view"),
        (
            V3,
            "edit password --id 1 --password x -e 2024-07-05",
            "account/editPass",
        ),
        (V3, "new category -n harness_cat -e d", "category/create"),
        (V3, "edit category --id 1 -n x -e y", "category/view"),
        (V3, "remove category --id 1", "category/delete"),
        (V3, "new client -n harness_client -e d", "client/create"),
        (V3, "edit client --id 1 -n x -e y", "client/view"),
        (V3, "remove client --id 1", "client/delete"),
        (
            V3,
            "new password -n harness_account -l h -i 1 -a 1 -g 0 -o n -p pw",
            "account/create",
        ),
        (V3, "remove account -i 1", "account/delete"),
        (V2, "search -p test-ssh", "getAccountSearch"),
        (V2, "search -i 1 -p x", "getAccountData"),
        (V2, "new category -n harness_cat -e d", "addCategory"),
        (V2, "remove category --id 1", "deleteCategory"),
        (V2, "new client -n harness_client -e d", "addCustomer"),
        (V2, "remove client --id 1", "deleteCustomer"),
        (
            V2,
            "new password -n harness_account -l h -i 1 -a 1 -g 0 -o n -p pw",
            "addAccount",
        ),
        (V2, "remove account -i 1", "deleteAccount"),
    ]
    .into_iter()
    .map(|(config, command, expected)| Step {
        config,
        command,
        expected,
    })
    .collect()
}

const V3: &str = "tests/fixtures/config_dead_v3.json";
const V2: &str = "tests/fixtures/config_dead_v2.json";

/// Warnings the CLI currently produces. Pinned so a new deviation cannot slip in unnoticed;
/// adding one here is a decision, not an accident.
const EXPECTED_WARNINGS: [&str; 4] = [
    "2.1 addCategory tokenPass",
    "2.1 addCustomer tokenPass",
    "3.1 category/create tokenPass",
    "3.1 client/create tokenPass",
];

#[test]
fn cli_requests_match_the_documented_params() {
    let v3 = load_table("tests/fixtures/docs_v3.json");
    let v2 = load_table("tests/fixtures/docs_v2.json");
    println!("v3 table from {}", v3.source);
    println!("v2 table from {}", v2.source);

    let mut failures = Vec::new();
    let mut warnings = Vec::new();

    for step in steps() {
        let table = if step.config == V3 { &v3 } else { &v2 };
        let stdout = run_cli(step.config, step.command);
        let (requests, problems) = requests_in_output(&stdout);
        failures.extend(problems.into_iter().map(|p| format!("{}: {p}", step.command)));

        if requests.is_empty() {
            failures.push(format!("{} sent nothing", step.command));
            continue;
        }

        for request in &requests {
            let findings = check(table, request);
            let hard = findings.iter().any(Finding::is_hard);

            println!(
                "{} {} {} params=[{}]",
                if hard {
                    "FAIL"
                } else if findings.is_empty() {
                    "PASS"
                } else {
                    "WARN"
                },
                table.version,
                request.method,
                request
                    .params
                    .keys()
                    .filter(|k| *k != "authToken")
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(",")
            );

            for finding in &findings {
                println!("     {} {} {}", finding.kind, finding.param, finding.detail);
                if !finding.is_hard() {
                    warnings.push(format!("{} {} {}", table.version, request.method, finding.param));
                }
            }

            if hard {
                failures.push(format!("{}:{}", step.command, request.method));
            }

            if request.method != step.expected {
                failures.push(format!(
                    "{} sent {} instead of {}",
                    step.command, request.method, step.expected
                ));
            }
        }
    }

    warnings.sort();
    assert_eq!(warnings, EXPECTED_WARNINGS.to_vec(), "warning set changed");
    assert!(failures.is_empty(), "failures: {}", failures.join(", "));
}

/// v2 has no single-entity fetch and no password change. These paths report `NotSupported`
/// without sending a request (issue #16).
#[test]
fn not_supported_paths_send_no_request() {
    let mut failures = Vec::new();

    for (command, expected_message) in [
        (
            "edit password --id 1 --password x -e 2024-07-05",
            "Changing a password is not supported by this version of the API",
        ),
        (
            "edit category --id 1 -n x -e y",
            "Fetching a single category is not supported by this version of the API",
        ),
        (
            "edit client --id 1 -n x -e y",
            "Fetching a single client is not supported by this version of the API",
        ),
    ] {
        let stdout = run_cli(V2, command);

        if !stdout.contains(expected_message) || !requests_in_output(&stdout).0.is_empty() {
            failures.push(format!("{command}: {stdout}"));
        } else {
            println!("PASS v2 not-supported {command}");
        }
    }

    assert!(failures.is_empty(), "failures: {}", failures.join(", "));
}

/// Every documented method must be either driven by a step or recorded as a gap with a
/// reason, so the gap list cannot drift away from the tables (issues #16 and #17).
#[test]
fn every_documented_method_is_checked_or_recorded_as_a_gap() {
    let gaps = [
        ("v3", "tag/search", "no tags in the CLI"),
        ("v3", "tag/view", "no tags in the CLI"),
        ("v3", "tag/create", "no tags in the CLI"),
        ("v3", "tag/edit", "no tags in the CLI"),
        ("v3", "tag/delete", "no tags in the CLI"),
        ("v3", "userGroup/search", "no user groups in the CLI"),
        ("v3", "userGroup/view", "no user groups in the CLI"),
        ("v3", "userGroup/create", "no user groups in the CLI"),
        ("v3", "userGroup/edit", "no user groups in the CLI"),
        ("v3", "userGroup/delete", "no user groups in the CLI"),
        ("v3", "config/backup", "no backup in the CLI"),
        ("v3", "config/export", "no export in the CLI"),
        ("v3", "account/edit", "needs an interactive selection"),
        ("v3", "category/search", "needs an interactive selection"),
        ("v3", "client/search", "needs an interactive selection"),
        ("v3", "account/viewPass", "needs a server reply first"),
        ("v3", "category/edit", "needs a server reply first"),
        ("v3", "client/edit", "needs a server reply first"),
        ("v2", "backup", "no backup in the CLI"),
        ("v2", "getCategories", "needs an interactive selection"),
        ("v2", "getCustomers", "needs an interactive selection"),
        ("v2", "getAccountPassword", "needs a server reply first"),
    ];

    let checked = steps();
    let mut failures = Vec::new();

    for version in ["v3", "v2"] {
        let table = load_table(&format!("tests/fixtures/docs_{version}.json"));

        let driven: BTreeSet<&str> = checked
            .iter()
            .filter(|s| (version == "v3") == (s.config == V3))
            .map(|s| s.expected)
            .collect();

        let gap_names: BTreeSet<&str> = gaps
            .iter()
            .filter(|(v, _, _)| *v == version)
            .map(|(_, method, _)| *method)
            .collect();

        for method in table.methods.keys() {
            if !driven.contains(method.as_str()) && !gap_names.contains(method.as_str()) {
                failures.push(format!(
                    "{version} {method} is documented but neither driven nor recorded"
                ));
            }
        }

        for (v, method, reason) in gaps.iter().filter(|(v, _, _)| *v == version) {
            if table.methods.contains_key(*method) {
                println!("NOT CHECKED {v} {method} ({reason})");
            } else {
                failures.push(format!("{v} {method} is recorded as a gap but is not documented"));
            }
        }
    }

    assert!(failures.is_empty(), "failures: {}", failures.join(", "));
}

#[test]
fn checker_reports_a_deliberate_mismatch() {
    let good = load_table("tests/fixtures/docs_v3.json");

    let mut bad = good.clone();
    let count = bad
        .methods
        .get_mut("account/search")
        .expect("account/search should be in the table")
        .params
        .iter_mut()
        .find(|p| p.name == "count")
        .expect("count should be documented for account/search");
    count.required = Some(true);

    let request = SentRequest {
        method: "account/search".to_string(),
        params: [
            ("authToken".to_string(), "t".to_string()),
            ("text".to_string(), "x".to_string()),
        ]
        .into_iter()
        .collect(),
    };

    assert!(check(&good, &request).is_empty(), "the good table should pass");

    let findings = check(&bad, &request);
    assert_eq!(findings.len(), 1, "the mismatch should produce one finding");
    assert_eq!(findings[0].kind, Kind::MissingRequired);
    assert_eq!(findings[0].param, "count");
}

#[test]
fn checker_flags_undocumented_and_type_mismatch() {
    let table = load_table("tests/fixtures/docs_v3.json");

    let request = SentRequest {
        method: "account/search".to_string(),
        params: [
            ("authToken".to_string(), "t".to_string()),
            ("text".to_string(), "x".to_string()),
            ("count".to_string(), "abc".to_string()),
            ("unknownThing".to_string(), "1".to_string()),
        ]
        .into_iter()
        .collect(),
    };

    let findings = check(&table, &request);
    assert!(
        findings
            .iter()
            .any(|f| f.kind == Kind::UndocumentedParam && f.param == "unknownThing")
    );
    assert!(findings.iter().any(|f| f.kind == Kind::TypeMismatch && f.param == "count"));
}

#[test]
fn checker_flags_a_documented_param_replaced_by_another_name() {
    let table = load_table("tests/fixtures/docs_v2.json");

    let request = SentRequest {
        method: "getAccountData".to_string(),
        params: [
            ("authToken".to_string(), "t".to_string()),
            ("id".to_string(), "1".to_string()),
            ("tokenPass".to_string(), "[REDACTED]".to_string()),
        ]
        .into_iter()
        .collect(),
    };

    let findings = check(&table, &request);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, Kind::ParamNameDivergence);
    assert_eq!(findings[0].param, "userPass vs tokenPass");
    assert!(findings[0].is_hard());
}
