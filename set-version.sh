#!/usr/bin/env bash

VERSION="$(cargo read-manifest | jq .version | sed "s/\"//g")"
sed -i -E "s#-[0-9]+\.[0-9]+\.[0-9]_#-${VERSION}_#g" README.md
sed -i -E "s#/[0-9]+\.[0-9]+\.[0-9]/#/${VERSION}/#g" README.md
