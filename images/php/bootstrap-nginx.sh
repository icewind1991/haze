#!/bin/sh

touch /var/log/nginx/access.log
touch /var/log/nginx/error.log

tail --follow --retry /var/log/nginx/*.log &

UID=${UID:-1000}
GID=${GID:-1000}

if [ $(getent group $GID) ]; then
  groupadd haze
  EXTRA_GROUP=" -G haze"
else
  groupadd -g $GID haze
  EXTRA_GROUP=""
fi
useradd -u $UID -g $GID $EXTRA_GROUP haze
chown -R haze:$GID /home/haze

if [ -f "/var/run/docker.sock" ]; then
  groupadd docker -g $(stat --format "%g" /var/run/docker.sock)
  usermod -a -G docker haze
fi

/usr/local/sbin/php-fpm &
nginx
