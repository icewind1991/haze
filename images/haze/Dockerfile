ARG PHP_VERSION
FROM icewind1991/haze-php:$PHP_VERSION

RUN DEBIAN_FRONTEND=noninteractive ;\
	apt-get update && \
	apt-get install --assume-yes \
    cron \
    redis-server \
    smbclient \
    npm \
		sudo \
		wget \
		attr \
		git \
    neovim \
    nano \
    sqlite3 \
    s3cmd \
    gdb \
    python3-pip \
    procps \
    rsnapshot \
#   chromium \
    ncat && \
  apt-get install --assume-yes libgtk2.0-0 libgtk-3-0 libgbm-dev libnotify-dev libnss3 libxss1 libasound2 libxtst6 xauth xvfb && \
  curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64-2.1.35.zip" -o "awscliv2.zip" && \
  unzip awscliv2.zip && \
  ./aws/install && \
  wget https://phar.phpunit.de/phpunit-9.phar -O /usr/local/bin/phpunit.phar && \
  curl -L https://github.com/harness/drone-cli/releases/latest/download/drone_linux_amd64.tar.gz | tar zx  && \
  install -t /usr/bin drone && \
  wget https://github.com/vimeo/psalm/releases/latest/download/psalm.phar -O /usr/local/bin/psalm && \
  chmod +x /usr/local/bin/psalm && \
  touch /var/log/rsnapshot.log && \
  chmod 0777 /var/log/rsnapshot.log && \
  chmod 0777 /var/cache/rsnapshot

ADD misc/collaboraonline.sources /etc/apt/sources.list.d
RUN DEBIAN_FRONTEND=noninteractive ;\
  wget -q -O - https://packages.blackfire.io/gpg.key | apt-key add - && \
  echo "deb http://packages.blackfire.io/debian any main" >> /etc/apt/sources.list.d/blackfire.list && \
  wget https://collaboraoffice.com/downloads/gpg/collaboraonline-release-keyring.gpg -O /usr/share/keyrings/collaboraonline-release-keyring.gpg && \
  apt-get update && \
	apt-get install --assume-yes blackfire

RUN curl https://dl.min.io/client/mc/release/linux-amd64/mc -o /usr/bin/mc && \
    chmod +x /usr/bin/mc

ADD configs /root/
ADD configs/nginx-app.conf /etc/nginx/

RUN mkdir --parent /var/log/cron
ADD configs/cron.conf /etc/oc-cron.conf

ADD misc/bootstrap.sh misc/occ misc/tests misc/phpunit misc/install misc/occ misc/integration misc/chromium-no-sandbox /usr/local/bin/
ADD configs/sudoers /etc/sudoers.d/haze
ADD configs/.s3cfg configs/.gdbinit configs/.aws /home/haze/
ADD rsnapshot.conf /etc/rsnapshot.conf
ADD redis-certificates /redis-certificates

ENV WEBROOT /var/www/html
ENV EDITOR /usr/bin/neovim

ENTRYPOINT  ["bootstrap.sh"]
