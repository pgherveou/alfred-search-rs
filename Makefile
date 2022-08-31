.PHONY: update-schema
update-schema:
	curl https://docs.github.com/public/schema.docs.graphql -o schema.graphql
