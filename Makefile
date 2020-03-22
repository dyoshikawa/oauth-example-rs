.PHONY: client server resource

client:
	cargo run --bin client

server:
	cargo run --bin authorization_server

resource:
	cargo run --bin protected_resource
