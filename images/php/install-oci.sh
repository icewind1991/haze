#!/bin/sh

PHP=$(echo "$PHP_VERSION" | cut -c -3)

echo "php $PHP"

case $PHP in
  "7.4") OCI_VERSION="-2.2.0" ;;
  "8.0") OCI_VERSION="-3.0.1" ;;
  "8.1") OCI_VERSION="-3.2.1" ;;
   *) status=$status ;;
esac

echo "using oci8$OCI_VERSION"

mkdir /opt/oracle
cd /opt/oracle
wget https://download.oracle.com/otn_software/linux/instantclient/2110000/instantclient-basiclite-linux.x64-21.10.0.0.0dbru.zip
wget https://download.oracle.com/otn_software/linux/instantclient/2110000/instantclient-sdk-linux.x64-21.10.0.0.0dbru.zip
unzip instantclient-basiclite-linux.x64-21.10.0.0.0dbru.zip
unzip instantclient-sdk-linux.x64-21.10.0.0.0dbru.zip
rm instantclient*.zip
echo /opt/oracle/instantclient_21_10 > /etc/ld.so.conf.d/oracle-instantclient.conf
ldconfig
pecl install -D 'with-oci8="instantclient,/opt/oracle/instantclient_21_10"' oci8$OCI_VERSION