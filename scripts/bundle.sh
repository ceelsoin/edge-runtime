#!/bin/bash

# Bundle all TypeScript files in examples directory
for file in examples/**/*.ts; do
    if [ -f "$file" ]; then
        output="${file%.ts}.eszip"
        echo "Bundling $file -> $output"
        ./target/debug/thunder bundle -e "$file" -o "$output"

        curl -X POST http://localhost:9000/_internal/functions \
        -H "x-function-name: $(basename "$file" .ts)" \
        --data-binary @"$output"
    fi
done

