# Builds the WebAssembly module and serves the static web shell with nginx.
# The image contains no game data: players drop their own copy onto the page, or the
# host mounts the extracted files at /usr/share/nginx/html/game (see docs in README).
FROM rust:1-bookworm AS build
ARG GIT_COMMIT=development
RUN rustup target add wasm32-unknown-unknown
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tools ./tools
RUN cargo build --release --target wasm32-unknown-unknown \
    && ls -la target/wasm32-unknown-unknown/release/cyber_empires.wasm

FROM nginx:1.27-alpine
ARG GIT_COMMIT=development
LABEL org.opencontainers.image.source="https://github.com/8exgh/cyber-empires" \
      org.opencontainers.image.revision="${GIT_COMMIT}"
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY web/index.html web/app.js web/style.css /usr/share/nginx/html/
COPY --from=build /src/target/wasm32-unknown-unknown/release/cyber_empires.wasm /usr/share/nginx/html/cyber_empires.wasm
# Stamp the release into the script and stylesheet URLs so CDN edges and browsers
# never serve a previous release's app.js against a new build.
RUN printf '%s\n' "${GIT_COMMIT}" > /usr/share/nginx/html/commit.txt \
    && v=$(printf '%s' "${GIT_COMMIT}" | cut -c1-12) \
    && sed -i "s#href=\"style.css\"#href=\"style.css?v=${v}\"#; s#src=\"app.js\"#src=\"app.js?v=${v}\"#" /usr/share/nginx/html/index.html \
    && grep -q "app.js?v=${v}" /usr/share/nginx/html/index.html \
    && mkdir -p /usr/share/nginx/html/game
EXPOSE 80
