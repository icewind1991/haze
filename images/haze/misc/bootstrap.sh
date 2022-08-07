#!/bin/sh

touch /var/log/nginx/access.log
touch /var/log/nginx/error.log
touch /var/log/cron/owncloud.log

cp /root/config.php /var/www/html/config/config.php

if [ "$SQL" = "mysql" ]
then
	cp /root/autoconfig_mysql.php /var/www/html/config/autoconfig.php
fi

if [ "$SQL" = "mariadb" ]
then
	cp /root/autoconfig_mariadb.php /var/www/html/config/autoconfig.php
fi

if [ "$SQL" = "pgsql" ]
then
	cp /root/autoconfig_pgsql.php /var/www/html/config/autoconfig.php
fi

if [ "$SQL" = "oci" ]
then
	cp /root/autoconfig_oci.php /var/www/html/config/autoconfig.php
fi

UID=${UID:-www-data}
GID=${GID:-www-data}

echo "Running as $UID:$GID"

chown -R $UID:$GID /var/www/html/data /var/www/html/config
chown $UID:$GID /var/www/html/core/skeleton /var/www/html/build/integration/vendor /var/www/html/build/integration/composer.lock /var/www/html/build/integration/output /var/www/html/build/integration/work /var/www/html/core/skeleton /var/www/.composer/cache /var/www/html/apps/spreed/tests/integration/vendor/composer

echo "{}" > /var/www/html/build/integration/composer.lock

echo "Starting server using $SQL database…"

tail --follow --retry /var/log/nginx/*.log /var/log/cron/owncloud.log &

if [ -n "$S3" ]
then
	sed -i '/\/\/PLACEHOLDER/ r /root/s3.php' /var/www/html/config/config.php
fi

if [ -n "$S3MB" ]
then
	sed -i '/\/\/PLACEHOLDER/ r /root/s3mb.php' /var/www/html/config/config.php
fi

if [ -n "$SWIFT" ]
then
    sed -i '/\/\/PLACEHOLDER/ r /root/swift.php' /var/www/html/config/config.php
fi

if [ -n "$SWIFTV3" ]
then
    sed -i '/\/\/PLACEHOLDER/ r /root/swiftv3.php' /var/www/html/config/config.php
fi

if [ -n "$AZURE" ]
then
    sed -i '/\/\/PLACEHOLDER/ r /root/azure.php' /var/www/html/config/config.php
fi

if [ -n "$BLACKFIRE_SERVER_ID" ]
then
  sh -c '
    yes | blackfire agent:config --server-id=$BLACKFIRE_SERVER_ID --server-token=$BLACKFIRE_SERVER_TOKEN
    mkdir /var/run/blackfire/
    BLACKFIRE_LOG_LEVEL=4 BLACKFIRE_LOG_FILE=/var/log/agent.log blackfire agent &
  '&
fi

crontab /etc/oc-cron.conf

/usr/sbin/cron -f &
/usr/bin/redis-server --protected-mode no &
/usr/local/bin/bootstrap-nginx.sh
