# Development

This document describes the process for running this application on your local computer.

## Getting started

It runs on macOS, Windows, and Linux environments.

```sh
git clone https://github.com/ggnosh/syspass-cli.git
cd syspass-cli
cp resources/test_config.json test_config.json
cp resources/test_config_v2.json test_config_v2.json
docker-compose -p syspass up -d
```

### Syspass v3 setup

Open up https://localhost:5000/ and finish the installation for a syspass instance.

| Setting name       | Value      |
|--------------------|------------|
| Admin user         | admin      |
| Password           | syspass    |
| Master password    | *any*      |
| Db access user     | root       |
| Db access password | syspass    |
| Database name      | syspass    |
| Database server    | syspass-db |

After installing, you can log in using the same address and add API authorization for the current user.

Update [test_config.json](test_config.json) with the new authorization token.

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
| Db access user     | root         |
| Db access password | syspass      |
| Database name      | syspass      |
| Database server    | syspass-dbv2 |

After installing, you can log in using the same address and add API authorization for the current user.

Update [test_config_v2.json](test_config_v2.json) with the new authorization token.

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
