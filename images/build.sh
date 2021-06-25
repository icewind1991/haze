#!/bin/bash

versions=("7.2" "7.3" "7.4" "8.0")

for version in "${versions[@]}"; do
  echo "building haze-php-$version"
  docker build -t "icewind1991/haze-php:$version" -f "php/Dockerfile.$version" php
done

for version in "${versions[@]}"; do
  echo "building haze-$version"
  cp haze/Dockerfile.tmpl haze/Dockerfile
  sed -i "s/<version>/$version/" haze/Dockerfile
  docker build -t "icewind1991/haze:$version" -f "haze/Dockerfile" haze
done

docker build -t "icewind1991/haze-ldap" -f "ldap/Dockerfile" ldap