# crabby-merge

A port/ripoff of [polly-merge](https://github.com/noahp/polly-merge) with a little speedup.
Basically just an uncreative excuse to play with async/await in Rust.

TODO:
* CI
* check PR comments too
* clean up `search_owned_prs` and `search_approved_prs` using iterator to capture joinhandles?
* reasonable error handling and meaningful error messages
* unit tests
* merge after
* configure merge trigger using config file
* allow multiple regexes in a regex set
* logging
* remove dotenv dependency
* comments and function docs
* use hyper directly
