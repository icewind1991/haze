#!/bin/sh

touch /var/log/nginx/access.log
touch /var/log/nginx/error.log

tail --follow --retry /var/log/nginx/*.log &

UID=${UID:-$(id -u)}
GID=${GID:-$(id -g)}

groupadd -g $GID haze
useradd -u $UID -g $GID haze

cat /usr/local/etc/php-fpm.conf

/usr/local/sbin/php-fpm &
/etc/init.d/nginx start
