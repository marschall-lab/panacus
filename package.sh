#!/usr/bin/env bash

# Best way to use this script is to run it in the form:
# -------------------------------------------------------
# PANACUS_TARGET="x86_64-unknown-linux-musl" ./package.sh
# -------------------------------------------------------
# Output is in the pkg directory

NAME="panacus"
EXEC="panacus"
VERSION="$(cargo read-manifest | jq .version | sed "s/\"//g")"
ARCH="${PANACUS_TARGET}"
# ARCH="x86_64-unknown-linux-musl"
# ARCH="x86_64-apple-darwin"
# ARCH="aarch64-apple-darwin"

echo "Packaging ${NAME}, ${EXEC}, ${VERSION}, ${ARCH}"
cargo fmt && \
cargo check && \
cargo test || exit 1

echo "Building release"
cargo build --release --target ${ARCH} || exit 1
echo "Finished building release"


if [ ! -d ./pkg ]; \
then \
    mkdir ./pkg; \
fi

if [ -d ./pkg/${NAME}-${VERSION}_${ARCH} ]; \
then \
    echo "Current version number already exists! Removing old files!"; \
    rm -rf ./pkg/${NAME}-${VERSION}_${ARCH}; \
fi

mkdir ./pkg/${NAME}-${VERSION}_${ARCH}

cp -r ./scripts ./pkg/${NAME}-${VERSION}_${ARCH}/
cp -r ./docs ./pkg/${NAME}-${VERSION}_${ARCH}/

mkdir ./pkg/${NAME}-${VERSION}_${ARCH}/bin
cp target/${ARCH}/release/${EXEC} ./pkg/${NAME}-${VERSION}_${ARCH}/bin/
strip ./pkg/${NAME}-${VERSION}_${ARCH}/bin/${EXEC}
ln -s ../scripts/$NAME-visualize.py ./pkg/${NAME}-${VERSION}_${ARCH}/$NAME-visualize

cp LICENSE ./pkg/${NAME}-${VERSION}_${ARCH}/
cp LICENSES ./pkg/${NAME}-${VERSION}_${ARCH}/
bash ./set-version.sh
cp README.md ./pkg/${NAME}-${VERSION}_${ARCH}/
cp -r examples ./pkg/${NAME}-${VERSION}_${ARCH}/

cd ./pkg && tar -czf ./${NAME}-${VERSION}_${ARCH}.tar.gz ./${NAME}-${VERSION}_${ARCH}
echo "Cleaning up ./pkg/${NAME}-${VERSION}_${ARCH}"
rm -rf ./${NAME}-${VERSION}_${ARCH}
