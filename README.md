# crabby-merge

A ~~ripoff~~ port of [polly-merge](https://github.com/noahp/polly-merge) into async Rust for some
marginal speedup. Mostly just an uncreative excuse to try out async/await in Rust 👨🏽‍🎓

## Configuration

Set a few environment variables:

* `BITBUCKET_URL` - base URL of the Bitbucket server to query
* `BITBUCKET_API_TOKEN` - API token for user authentication
* `CRABBY_MERGE_TRIGGER` - (optional) Regex string to look in PR descriptions for that indicates
that a PR is ready to merge. Must be on its own line in the PR description. Defaults to `:shipit:`.

## TODO

More things to steal from polly-merge someday:

* Allow blocking merge on another PR ("merge after")
* Look for merge trigger in PR comments, too
* Log to file
