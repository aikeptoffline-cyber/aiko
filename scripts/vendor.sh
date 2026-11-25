#!/usr/bin/env sh
# Fetches the JS libraries for the optional 3D (VRM) mode into ui/vendor, so the app runs offline.
# Same files as setup.ps1 on Windows.
set -e
cd "$(dirname "$0")/.."
cdn=https://cdn.jsdelivr.net/npm
get() { mkdir -p "$(dirname "$2")"; curl -fsSL "$cdn/$1" -o "$2"; echo "fetched $2"; }
get three@0.169.0/build/three.module.js ui/vendor/three/build/three.module.js
get three@0.169.0/examples/jsm/loaders/GLTFLoader.js ui/vendor/three/examples/jsm/loaders/GLTFLoader.js
get three@0.169.0/examples/jsm/utils/BufferGeometryUtils.js ui/vendor/three/examples/jsm/utils/BufferGeometryUtils.js
get @pixiv/three-vrm@3.1.0/lib/three-vrm.module.min.js ui/vendor/three-vrm.module.min.js
