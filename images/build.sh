#!/usr/bin/env bash

set -e

export DOCKER_BUILDKIT=1

versions=("8.0" "8.1" "8.2")

for version in "${versions[@]}"; do
  echo "building haze-php-$version"
  docker build --build-arg PHP_VERSION=$version -t "icewind1991/haze-php:$version" -f "php/Dockerfile" php
  echo "building haze-php-$version-dbg"
  docker build --build-arg BASE_IMAGE=icewind1991/php-dbg --build-arg PHP_VERSION=$version -t "icewind1991/haze-php:$version-dbg" -f "php/Dockerfile" php
done

for version in "${versions[@]}"; do
  echo "building haze-$version"
  docker build --build-arg PHP_VERSION=$version -t "icewind1991/haze:$version" -f "haze/Dockerfile" haze
  echo "building haze-$version-dbg"
  docker build --build-arg PHP_VERSION=$version-dbg -t "icewind1991/haze:$version-dbg" -f "haze/Dockerfile" haze
done

docker build -t "icewind1991/haze-ldap" -f "ldap/Dockerfile" ldap