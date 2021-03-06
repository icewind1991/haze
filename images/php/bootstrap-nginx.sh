#!/bin/sh

touch /var/log/nginx/access.log
touch /var/log/nginx/error.log

tail --follow --retry /var/log/nginx/*.log &

UID=${UID:-1000}
GID=${GID:-1000}

groupadd -g $GID haze
useradd -m -u $UID -g $GID haze
chown -R haze:haze /home/haze

/usr/local/sbin/php-fpm &
/etc/init.d/nginx start
