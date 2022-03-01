ARG PHP_VERSION
FROM icewind1991/haze-php:$PHP_VERSION

RUN DEBIAN_FRONTEND=noninteractive ;\
    echo 'deb http://http.debian.net/debian buster-backports main' >> /etc/apt/sources.list && \
	apt-get update && \
	apt-get install --assume-yes \
		cron \
		redis-server \
		smbclient \
		sudo \
		wget \
		attr \
		git \
        neovim \
        nano \
        sqlite3 \
        s3cmd \
        python3-pip && \
    curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64-2.1.35.zip" -o "awscliv2.zip" && \
    unzip awscliv2.zip && \
    ./aws/install && \
    pip3 install awscli-plugin-endpoint && \
    wget https://phar.phpunit.de/phpunit-8.phar -O /usr/local/bin/phpunit.phar && \
    curl -L https://github.com/harness/drone-cli/releases/latest/download/drone_linux_amd64.tar.gz | tar zx  && \
    install -t /usr/bin drone

RUN DEBIAN_FRONTEND=noninteractive ;\
    apt-get update && \
    apt-get install --assume-yes \
        npm \
        chromium

ADD configs/autoconfig_mariadb.php configs/autoconfig_mysql.php configs/autoconfig_pgsql.php configs/autoconfig_oci.php configs/s3.php configs/s3mb.php configs/swift.php configs/swiftv3.php configs/azure.php configs/config.php /root/
ADD configs/nginx-app.conf /etc/nginx/

RUN mkdir --parent /var/log/cron
ADD configs/cron.conf /etc/oc-cron.conf

ADD misc/bootstrap.sh misc/occ misc/tests misc/phpunit misc/install misc/occ misc/integration /usr/local/bin/
ADD configs/sudoers /etc/sudoers.d/haze
ADD configs/s3cfg /home/haze/.s3cfg
ADD configs/aws /home/haze/.aws

ENV WEBROOT /var/www/html

ENTRYPOINT  ["bootstrap.sh"]
