# crabby-merge

Scans open Bitbucket pull requests for a configurable trigger string and merges them for you.

This is largely a ~~ripoff~~ port of [polly-merge](https://github.com/noahp/polly-merge) into async
Rust. There's a bit of speedup gained, but this is mostly just an uncreative excuse to try out
async/await in Rust 👨🏽‍🎓

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

## TODO

* Allow blocking merge on another PR ("merge after" in polly-merge)
