# crabby-merge

[![build status](https://img.shields.io/github/actions/workflow/status/kesyog/crabby-merge/rust.yml?branch=master&style=flat-square)](https://github.com/kesyog/crabby-merge/actions/workflows/rust.yml)
[![crates.io](https://img.shields.io/crates/v/crabby-merge?style=flat-square)](https://crates.io/crates/crabby-merge)
[![Apache 2.0 license](https://img.shields.io/github/license/kesyog/crabby-merge?style=flat-square)](./LICENSE)

Scans open Bitbucket pull requests for a configurable trigger string and merges them for you.

This is largely a ~~ripoff~~ port of [polly-merge](https://github.com/noahp/polly-merge) into async
Rust. There's a bit of speedup gained, but this is mostly just an uncreative excuse to try out
async/await in Rust 👨🏽‍🎓

## Installation

Install via [Cargo](https://rustup.rs):

```sh
cargo install crabby-merge
```

## Usage

Ideally, you'd schedule crabby-merge to be run periodically. To accomplish this with [cron](https://en.wikipedia.org/wiki/Cron),
on a Unix-like machine, run `crontab -e` and add an entry like:

```text
# Schedule crabby-merge to run every two minutes
*/2 * * * * $HOME/.cargo/bin/crabby-merge
```

## Configuration

### TOML

In `$HOME/.crabby_merge.toml`:

```toml
# base URL of the Bitbucket server to query. Required.
bitbucket_url = "your URL goes here"
# API token for user authentication
bitbucket_api_token = "your token goes here"
# Trigger regex string to look for
merge_trigger = "^:shipit:$"
# Whether to check the pull request description for the trigger
check_description = true
# Whether to check pull request comments for the trigger. Only the user's own comments are searched.
check_comments = false
# Whether to include the user's own pull requests
check_own_prs = true
# Whether to search pull requests the user has approved
check_approved_prs = false
```

All fields are optional unless indicated. Values shown are the default values.

### Environment variables

Each of the TOML keys listed above can be prefixed with `CRABBY_MERGE` and provided as an
environment variable. Keys are case-insensitive.

For example, you can pass in the bitbucket API token as `CRABBY_MERGE_API_TOKEN=<your token here>`.

## Jenkins rebuild support

There is experimental support for rebuilding failed Jenkins builds whose name matches a provided
regex trigger. This is a sad workaround for flaky blocking tests. This is compile-time gated by the
`jenkins` feature, which is enabled by default.

To use it, add the following fields to your configuration file. If these fields aren't provided, the
retry functionality will be disabled at runtime.

```toml
jenkins_username = ""
jenkins_password = ""
# Regex trigger to search against the build name
jenkins_retry_trigger = ""
# Optional. Defaults to 10.
jenkins_retry_limit = ""
```
