#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
IMAGE_NAME="service-matrix-rust"
CONTAINER_NAME="service-matrix-rust"
GIT_SHA=$(git -C "$SCRIPT_DIR" rev-parse HEAD 2>/dev/null || printf 'unknown')

docker build \
  --build-arg "SERVICE_MATRIX_GIT_SHA=$GIT_SHA" \
  --tag "$IMAGE_NAME" \
  "$SCRIPT_DIR"

if docker container inspect "$CONTAINER_NAME" >/dev/null 2>&1; then
  docker container rm --force "$CONTAINER_NAME" >/dev/null
fi

CONTAINER_ID=$(docker run --detach \
  --name "$CONTAINER_NAME" \
  --restart unless-stopped \
  --publish 8080:8080 \
  --volume "$SCRIPT_DIR/data:/app/data" \
  --volume "$SCRIPT_DIR/resources:/app/resources" \
  "$IMAGE_NAME")

printf 'Deployed %s (%s) in the background.\n' "$CONTAINER_NAME" "$CONTAINER_ID"
printf 'Restart policy: unless-stopped\n'
printf 'Health: docker inspect --format={{.State.Health.Status}} %s\n' "$CONTAINER_NAME"
printf 'Logs: docker logs %s\n' "$CONTAINER_NAME"
