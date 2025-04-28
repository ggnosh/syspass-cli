set dotenv-load
alias v3 := run-v3
alias v2 := run-v2

default:
    @just --list

test-all:
    cargo nextest run -- --include-ignored

test:
    cargo nextest run

coverage-all:
    cargo +nightly llvm-cov nextest --html --output-dir coverage --no-cfg-coverage -- --include-ignored

coverage:
    cargo +nightly llvm-cov nextest --html --output-dir coverage --no-cfg-coverage

lint:
    cargo clippy -- -D warnings

lint-nightly:
    cargo +nightly clippy -- -D warnings

run-v2 *args='':
    cargo run -- -c test_config_v2.json {{args}}

run-v3 *args='':
    cargo run -- -c test_config.json {{args}}

build:
    cargo build --release

build-all:
    cargo build -q --target=x86_64-pc-windows-gnu --release
    cargo build -q --release

release:
    ./tools/release.sh

up:
    docker-compose up -d

down:
    docker-compose down --remove-orphans

docker-build:
    docker-compose build --no-cache

clean:
    cargo clean
