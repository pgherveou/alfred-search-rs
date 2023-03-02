# alfred-search-rs (wip)

Small cli utility to query various apis and send search results to Alfred.

It currently query the following APIs:
- Github
- crate.io

# caching

The cli caches results using SQLite, so that the cli can quickly return a list of items to Alfred.
It can also spawn a child fork of the process at a configured frequency to warmup the database in the background.

