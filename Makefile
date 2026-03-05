build:
	RUSTY_V8_MIRROR="https://github.com/denoland/rusty_v8/releases/download" cargo build 2>&1
run:
	RUSTY_V8_MIRROR="https://github.com/denoland/rusty_v8/releases/download" cargo run start 2>&1

test-js:
	RUSTY_V8_MIRROR="https://github.com/denoland/rusty_v8/releases/download" cargo run -- test --path "./tests/js/**/*.ts" --ignore "./tests/js/lib/**" 2>&1