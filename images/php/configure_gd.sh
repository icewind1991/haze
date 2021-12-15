#!/bin/sh

PHP=$(echo "$PHP_VERSION" | cut -c -3)

echo "php $PHP"

if [ "$PHP" = "7.2" ] || [ "$PHP" = "7.3" ]; then
  docker-php-ext-configure gd \
          --with-gd \
          --with-jpeg-dir \
          --with-png-dir \
          --with-zlib-dir \
          --with-freetype-dir
else
  docker-php-ext-configure gd \
          --enable-gd \
          --with-jpeg \
          --with-freetype
fi