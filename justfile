web-build:
    (cd eclss-web && trunk build)

run: web-build
    cargo run