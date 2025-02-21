ARG PHP_VERSION
ARG BASE_IMAGE=php
FROM ${BASE_IMAGE}:${PHP_VERSION}-fpm-bookworm
MAINTAINER  Robin Appelman <robin@icewind.nl>

RUN DEBIAN_FRONTEND=noninteractive ;\
    apt-get update && \
    apt-get install --assume-yes \
        bzip2 \
        nginx \
        libaio-dev \
        wget \
        unzip \
    && rm -rf /var/lib/apt/lists/*

ADD configure_gd.sh /

# php exceptions
RUN apt-get update \
    && apt-get install -y \
        libfreetype6-dev \
        libjpeg62-turbo-dev \
        libmcrypt-dev \
        libpng-dev \
        libpq5 \
        libpq-dev \
        libsqlite3-dev \
        libcurl4-openssl-dev \
        libicu-dev \
        libzip-dev \
        libonig-dev \
        libldap2-dev \
        libsmbclient-dev \
	      libgmp-dev \
    && /configure_gd.sh \
    && docker-php-ext-install gd \
    && docker-php-ext-install iconv zip pdo pdo_pgsql pdo_sqlite pgsql pdo_mysql intl curl mbstring pcntl ldap exif gmp \
    && pecl install inotify \
    && pecl install smbclient \
    && apt-get remove -y \
        libfreetype6-dev \
        libjpeg62-turbo-dev \
        libmcrypt-dev \
        libpng-dev \
        libpq-dev \
        libsqlite3-dev \
        libcurl4-openssl-dev \
        libicu-dev \
        libzip-dev \
        libonig-dev \
        libldap2-dev \
        libsmbclient-dev \
    && rm -rf /var/lib/apt/lists/*

RUN pecl install apcu \
    && pecl install xdebug \
    && pecl install redis \
    && export VERSION=`php -r "echo PHP_MAJOR_VERSION.PHP_MINOR_VERSION;"` \
    && curl -A "Docker" -o /tmp/blackfire-probe.tar.gz -D - -L -s https://blackfire.io/api/v1/releases/probe/php/linux/amd64/${VERSION} \
    && tar zxpf /tmp/blackfire-probe.tar.gz -C /tmp \
    && mv /tmp/blackfire-*.so `php -r "echo ini_get('extension_dir');"`/blackfire.so

ARG BASE_IMAGE
RUN sh -c 'if [ "$BASE_IMAGE" = "php" ]; then echo "extension=smbclient.so" > $PHP_INI_DIR/conf.d/smbclient.ini; fi' \
    && sh -c 'if [ "$BASE_IMAGE" = "php" ]; then echo "extension=blackfire.so" > $PHP_INI_DIR/conf.d/blackfire.ini; fi' \
    && echo "extension=inotify.so" > $PHP_INI_DIR/conf.d/inotify.ini \
    && echo "zend_extension=$(find /usr/local/lib/php/extensions/ -name xdebug.so)" > $PHP_INI_DIR/conf.d/xdebug.ini \
    && echo "xdebug.mode=debug,trace,profile" >> $PHP_INI_DIR/conf.d/xdebug.ini \
    && echo "xdebug.start_with_request=trigger" >> $PHP_INI_DIR/conf.d/xdebug.ini \
    && echo "xdebug.discover_client_host=true" >> $PHP_INI_DIR/conf.d/xdebug.ini \
    && echo "xdebug.client_host=hazehost" >> $PHP_INI_DIR/conf.d/xdebug.ini \
    && echo "xdebug.log_level=0" >> $PHP_INI_DIR/conf.d/xdebug.ini \
    && echo "xdebug.output_dir=/tmp/xdebug" >> $PHP_INI_DIR/conf.d/xdebug.ini \
    && echo "memory_limit = 512M" > $PHP_INI_DIR/conf.d/memory_limit.ini \
    && curl -sS https://getcomposer.org/installer | php -- --install-dir=/usr/bin --filename=composer

ADD install-oci.sh /
RUN /install-oci.sh \
    &&  echo "extension=oci8.so" > $PHP_INI_DIR/conf.d/oci8.ini


ADD apcu.ini opcache.ini redis.ini $PHP_INI_DIR/conf.d/

ADD nginx.conf nginx-app.conf /etc/nginx/


ADD php-fpm.conf /usr/local/etc/
ADD index.php /var/www/html/

ADD bootstrap-nginx.sh /usr/local/bin/

EXPOSE 80

ENTRYPOINT  ["bootstrap-nginx.sh"]
