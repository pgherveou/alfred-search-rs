.PHONY: update-schema
update-schema:
	curl https://docs.github.com/public/schema.docs.graphql -o schema.graphql

bootstrap:
	test -f .env || cp .env.example .env
	# initialize a new database with sqlite3 if it does not exist
	test -f ./db/alfred-search.db || sqlite3 ./db/alfred-search.db
	test -x $$(which sqlx) || cargo install sqlx
	sqlx migrate run
	cargo build


