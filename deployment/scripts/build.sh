#!/bin/bash
set -e

VERSION=${1:-latest}
REGISTRY=${HARBOR_REGISTRY:-harbor.local:30443}
IMAGE="$REGISTRY/isa/blockchain-node:$VERSION"

echo "Building isA_Chain node: $IMAGE"

# Build from repo root
cd "$(dirname "$0")/../.."

docker build -t "$IMAGE" -f core/blockchain/Dockerfile .

echo "Push with: docker push $IMAGE"
echo "Or: $0 $VERSION && docker push $IMAGE"
