#!/usr/bin/env bash

set -e

versions=("8.1" "8.2" "8.0-dbg" "8.1-dbg" "8.2-dbg")

for version in "${versions[@]}"; do
  docker push "icewind1991/haze-php:$version"
done

for version in "${versions[@]}"; do
  docker push "icewind1991/haze:$version"
done

docker push "icewind1991/haze-ldap"