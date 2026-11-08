# Usage history is global per user, not per config

Usage counts live in `~/.syspass/usage.json` for every run, regardless of which config file was passed with `-c`. A per-config usage file was considered and rejected: the ranking is meant to reflect what a person actually uses, and splitting it by config file fragments that data while writing files into arbitrary directories during development runs (`-c resources/test_config.json` would drop `usage.json` inside the repo).

The `dir` parameter on `Config::get_usage_data` and `Config::record_usage` is never passed in production. It exists so tests can point usage at a temporary directory; that is deliberate, not an oversight.

Recording happens in the CLI layer (`search.rs`) after an account has been successfully resolved for the user — list pick, single-hit search, and explicit `-i <id>` all count. The API clients read usage only to sort; they do not record it, so library calls from `edit` and `remove` are not counted as usage.
