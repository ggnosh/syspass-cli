#!/usr/bin/env sh
set -e

cd "$(dirname "$0")"

echo "New version number?"
read -r version

echo "Changing version number in Cargo.toml..."
sed -i "3s/.*/version = \"${version}\"/" ../syspass-cli/Cargo.toml

echo "Adding changelog line to CHANGELOG.md..."
sed -i "s/^# CHANGELOG/# CHANGELOG\n\n## ${version} - $(date +'%Y-%m-%d')/" ../CHANGELOG.md

./build.sh

echo "Commit changes..."
git commit -a -m "Version bump"

echo "Tagging version..."
git tag "v${version}"

echo "All done. Push and go"
