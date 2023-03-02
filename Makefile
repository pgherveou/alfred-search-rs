.PHONY: update-schema
update-schema:
	curl https://docs.github.com/public/schema.docs.graphql -o schema.graphql

bootstrap:
	test -f .env || cp .env.example .env
	cargo install sqlx
	sqlx migrate run
	cargo build


