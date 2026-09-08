# payserver-client - Leptos WASM frontend
#
# Expects a pre-built bundle from CI: dist/ (trunk build --release).
# There is no Rust toolchain in this image; nginx serves static files.

FROM nginx:alpine
COPY dist/ /usr/share/nginx/html/
COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
