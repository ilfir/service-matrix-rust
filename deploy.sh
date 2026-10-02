docker build --build-arg SERVICE_MATRIX_GIT_SHA="$(git rev-parse HEAD)" -t service-matrix-rust .
docker run --rm -p 8080:8080 \
  -v "$PWD/data:/app/data" \
  -v "$PWD/resources:/app/resources" \
  service-matrix-rust
  