# Development

This document describes the process for running this application on your local computer.

## Getting started

It runs on macOS, Windows, and Linux environments.

```sh
git clone https://github.com/ggnosh/syspass-cli.git
cd syspass-cli
cp resources/test_config.json test_config.json
cp resources/test_config_v2.json test_config_v2.json
docker compose up -d
```

### Syspass v3 setup

Open up https://localhost:5000/ and finish the installation for a syspass instance.

| Setting name       | Value      |
|--------------------|------------|
| Admin user         | admin      |
| Password           | syspass    |
| Master password    | *any*      |
| Db access user     | syspass    |
| Db access password | syspass    |
| Database name      | syspass    |
| Database server    | syspass-db |
| Hosting mode       | true       |

After installing, log in using the same address. Every API call is checked against the `AuthToken` table, which holds one authorization row per action for the user, so a fresh install needs those rows before the CLI can do anything. The API only checks 29 actions, and the UI token form offers exactly those, but picking them one at a time is slow; [scripts/add_api_authorizations.sql](scripts/add_api_authorizations.sql) adds 26 of them for the `admin` user using the token from [resources/test_config.json](resources/test_config.json):

```sh
docker exec -i syspass-db mariadb -u root -p"$MARIADB_ROOT_PASSWORD" syspass \
  < <(echo "SET @token = '$(jq -r .token resources/test_config.json)';"; cat scripts/add_api_authorizations.sql)
```

Generate one with `openssl rand -hex 32` and put it in that file if it is not set yet; the CLI config has to use the same value. Re-running the script is harmless since existing rows are skipped.

Three actions are deliberately left out — create them in the web UI (Authorizations page) with the same password as in `resources/test_config.json`: `New Account` (4), `View password` (7), `Edit Account Password` (8). Those rows store an encrypted vault and a hash key built from the password given when the authorization is created, which SQL cannot produce. A row inserted without them looks granted but fails the hash check in `ApiService::getMasterPassFromVault()` and returns 403.

Test authorization by running:

```sh
cargo run -- --config test_config.json new client -n test -e notes
cargo run -- --config test_config.json new category -n test -e notes
cargo run -- --config test_config.json new password -n test -u example.org -l test -o nothing -i 1 -a 1 -p password
cargo run -- --config test_config.json new password -n test-ssh -u ssh://localhost -l test -o nothing -i 1 -a 1 -p password
```

### Syspass v2 setup

Open up https://localhost:5001/ and finish the installation for a syspass instance.

| Setting name       | Value        |
|--------------------|--------------|
| Admin user         | admin        |
| Password           | syspass      |
| Master password    | *any*        |
| Db access user     | syspass      |
| Db access password | syspass      |
| Database name      | syspass      |
| Database server    | syspass-dbv2 |
| Hosting mode       | true         |

After installing, log in using the same address. The v2 API resolves the action from the method name (`SyspassApi::getActions`) and looks up the token per action in `authTokens`, so each action needs its own row. The v2 API implements 8 actions and the UI token form offers exactly those; [scripts/add_api_authorizations_v2.sql](scripts/add_api_authorizations_v2.sql) adds 6 of them for the `admin` user using the token from [resources/test_config_v2.json](resources/test_config_v2.json):

```sh
docker exec -i syspass-dbv2 mariadb -u root -p"$MARIADB_ROOT_PASSWORD" syspass \
  < <(echo "SET @token = '$(jq -r .token resources/test_config_v2.json)';"; cat scripts/add_api_authorizations_v2.sql)
```

Generate one with `openssl rand -hex 32` and put it in that file if it is not set yet; [test_config_v2.json](test_config_v2.json) has to use the same value. Re-running is harmless — v2 has no unique key on `(token, actionId)`, so the script skips rows that already exist.

Two actions are deliberately left out — create them in the web UI (Authorizations page) with the same password as in `resources/test_config_v2.json`: `New Account` (101) and `View password` (104). Those rows store an encrypted vault and a hash key built from the password given when the authorization is created, which SQL cannot produce. A row inserted without them looks granted but fails the hash check in `ApiBase::doAuth()` and returns 403.

If asked to upgrade:

```sh
docker exec syspass-appv2 cat sysPass/config/config.xml | grep upgrade
```

Test authorization by running:

```sh
cargo run -- --config test_config_v2.json new client -n test -e notes
cargo run -- --config test_config_v2.json new category -n test -e notes
cargo run -- --config test_config_v2.json new password -n test -u example.org -l test -o nothing -i 1 -a 1 -p password
cargo run -- --config test_config_v2.json new password -n test-ssh -u ssh://localhost -l test -o nothing -i 1 -a 1 -p password
```
