# builds the webapp
web-build:
    (cd eclss-web && trunk build)

web-serve:
    (cd eclss-web && trunk serve --no-default-features --features "middleware-local")

# builds the ECLSS ESP32 binary
build: web-build
    cargo run

run: web-build
    cargo run --release