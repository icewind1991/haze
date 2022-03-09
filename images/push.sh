#!/bin/bash

set -e

versions=("7.3" "7.4" "8.0" "8.1" "7.3-dbg" "7.4-dbg" "8.0-dbg" "8.1-dbg")

for version in "${versions[@]}"; do
  docker push "icewind1991/haze-php:$version"
done

for version in "${versions[@]}"; do
  docker push "icewind1991/haze:$version"
done

docker push "icewind1991/haze-ldap"