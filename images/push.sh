#!/bin/bash

versions=("7.2" "7.3" "7.4" "8.0")

for version in "${versions[@]}"; do
  docker push "icewind1991/haze-php:$version"
done

for version in "${versions[@]}"; do
  docker push "icewind1991/haze:$version"
done

docker push "icewind1991/haze-ldap"