# crabby-merge

A ~~ripoff~~ port of [polly-merge](https://github.com/noahp/polly-merge) into async Rust for some
speedup. Mostly just an uncreative excuse to try out async/await in Rust 👨🏽‍🎓

## Configuration

Set a few environment variables:

* `BITBUCKET_URL` - base URL of the Bitbucket server to query
* `BITBUCKET_API_TOKEN` - API token for user authentication
* `CRABBY_MERGE_TRIGGER` - (optional) Regex string to look for that indicates that a PR is ready to
merge. Must be on its own line in either the PR description or one of the user's own comments.
Defaults to `:shipit:`.

## TODO

* Allow configuration of features. Program run time is greatly extended by 1. checking approved
PR's and 2. checking PR comments. Making them optional would result in enormous speedup.
* Allow blocking merge on another PR ("merge after" in polly-merge)
* Miscellaneous cleanup and polish
