build:
	cargo build 2>&1
run:
	cargo run start 2>&1

test-js:
	cargo run -- test --path "./tests/js/**/*.ts" --ignore "./tests/js/lib/**" 2>&1

test: 
	cargo test
	cargo run -- test --path "./tests/js/**/*.ts" --ignore "./tests/js/lib/**" 2>&1
release:
	cargo build --release 2>&1
	cp target/release/deno-edge-runtime ./edge-runtime