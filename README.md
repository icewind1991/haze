# Haze

Hazy with a chance of clouds.

Easy setup and management of Nextcloud test instances using docker

## What

`haze` provides an easy way to setup Nextcloud test instances with a choice of php version, database server, optional s3
or ldap setup and more.

## Setup

### Requirements

- Docker

### Installation

- Grab a binary from the [github releases](https://github.com/icewind1991/haze/releases) and place it in your `$PATH`

### Config

Create a file `~/.config/haze/haze.toml` with the following options:

```toml
sources_root = "/path/to/nextcloud/sources"
```

See the [configuration section](#configuration) for more options.

### Quick examples

- Start a Nextcloud instance with `postgresql`, `php 8.1` and `s3` primary storage:

  ```bash
  haze start pgsq s3
  ```

- Start a Nextcloud instance with `sqlite`, `php 8.2` and an `smb` external storage:

  ```bash
  haze start 8.2 smb
  ```

- Run specific units test against an `oracle` database
  ```bash
  haze test oracle apps/dav/tests/unit/Connector/Sabre
  ```

## Managing instances

#### Start an instance

```bash
haze start [database] [php-version] [services]
```

Where `database` is one of `sqlite`, `mysql`, `mariadb`, `pgsql` or `oracle` with an optional version (e.g. `pgsql:12`),
defaults to `sqlite`.
And `php-version` is one of `8.1`, `8.2`, `8.3`, defaults to `8.1`. `7.3` and `7.4` and `8.0` are still supported but
the docker images for those versions aren't being updated anymore so they might be missing some newer features.

Each php version also comes with a `-dbg` variant that has php compiled in debug mode and can be used for debugging php
itself with gdb.

Additionally, you can use the following options when starting an instance:

- `s3`: setup an S3 server and configure to Nextcloud to use it as primary storage.
- `<path to app.tar.gz>`: by specifying the path to an app package this package will be extracted into the apps.
  directory of the new instance (overwriting any existing app code). This can be used to quickly test a packaged app.
- `ldap`: setup an LDAP server.
- `office`: setup a Nextcloud Office server.
- `onlyoffice` setup an onlyoffice document server.
- `push` setup [client push](https://github.com/nextcloud/notify_push).
- `smb`: setup a samba server for external storage use.
- `dav`: setup a WebDAV server for external storage use.
- `sftp`: setup a SFTP server for external storage use.
- `kaspersky`: setup a kaspersky scan engine server in http mode. (
  Requires [manually setting up the image](https://github.com/icewind1991/kaspersky-docker))
- `kaspersky-icap`: setup a kaspersky scan engine server in ICAP mode.
- `clamav-icap`: setup a clam av scanner in ICAP mode.
- `clamav-icap-tls`: setup a clam av scanner in ICAP mode with TLS encryption.
- `oc`: start an ownCloud instance in the same network.
- `imaginary`: start an Imaginary service and configure it for preview generation.
- `mail`: start an [smtp4dev](https://github.com/rnwood/smtp4dev) server and configure it the mail server.
- `redis-tls`: connect to redis over TLS.
- The name of any configured preset.

#### Run tests in a new instance

```bash
haze test [database] [php-version] [path]
```

Where `path` is a file or folder to run phpunit in, relative to the sources root.

### List running instances

```bash
haze
```

or

```bash
haze list
```

#### Remove all running instances

```bash
haze clean
```

## Controlling running instances

The following commands run against the most recently started instance and allow optionally providing a `match` to select
a specific instance by it's name.

#### Open an instance

```bash
haze [match] open
```

#### Open the database of an instance

```bash
haze [match] db
```

#### Execute a command on an instance

```bash
haze [match] exec [cmd]
```

If no `cmd` is specified it will launch `bash`

#### Create a new instance and run a command

```bash
haze [match] shell [cmd]
```

If no `cmd` is specified it will launch `bash`

#### Execute an occ command on an instance

```bash
haze [match] occ [cmd]
```

#### Connect to the database on an instance

```bash
haze [match] db
```

#### Show the logs of an instance

```bash
haze [match] logs
```

#### Stop an instance

```bash
haze [match] stop
```

#### Pin an instance

```bash
haze [match] pin
```

Pinned instances will not be removed by `haze clean`.

#### Unpin an instance

```bash
haze [match] unpin
```

#### Run a command with instance environment variables set

```bash
haze [match] env <cmd> [args]
```

Runs the provided command with `NEXTCLOUD_URL`, `DATABASE_URL` and `REDIS_URL` environment variables set for the matched
instance.

This is indented to run a local [push daemon](https://github.com/nextcloud/notify_push) against an instance.

## Federation

Multiple instances can reach each other by using their instance name as domain name to allow for testing federation
between instances.
Alternatively, you can setup the haze proxy and the proxied domains to get https support between instances.

## Proxy

By default, instances can be accessed by their IP. In order to get more memorable urls and allow supporting https,
haze comes with a builtin reverse proxy to allow using a wildcard domain.

### Requirements

- A domain name you can set wildcard DNS records for
- A reverse proxy like nginx or apache
- (optionally) a wildcard ssl certificate (can be acquiring using letsencrypt and dns verification)

### Setup

- Set a DNS record for `*.haze.exmaple.com` and `haze.example.com` pointing to your development machine. (127.0.0.1 will
  not work)
- Set the `proxy` configuration with your domain and desired listen endpoint
- Setup a service to run `haze proxy` in the background as your own user. A systemd user service is recommended.
- Configure your reverse proxy of choice to proxy `*.haze.example.com` and `haze.example.com` to the proxy's listen
  endpoint
- (optional) acquire a wildcard ssl certificate for your domain and set your reverse proxy to use it.
  This will be highly dependent on your DNS
  provider, [this](https://community.letsencrypt.org/t/dns-providers-who-easily-integrate-with-lets-encrypt-dns-validation/86438)
  lists some DNS providers and supported ACME clients.

### Usage

When the proxy is configured, generated urls for the instances will use a subdomain of the configured domain, e.g.
the `rolling-bees` instance will be available at `rolling-bees.haze.example.com`. Additionally, `haze.example.com` will
automatically point to the last created instance.

## Configuration

Configuration is loaded from `~/.config/haze/haze.toml` and has the following options

```toml
sources_root = "/path/to/sources" # path of the nextcloud sources. required
work_dir = "/path/to/temp/dir" # path to temporary directory. optional, defaults to "/tmp/haze"

[auto_setup] # optional
enabled = false # whether or not to automatically install nextcloud on `haze start`. optional, defaults to false
username = "foo" # username for admin user during auto setup. optional, defaults to "admin"
password = "bar" # password for admin user during auto setup. optional, defaults to "admin"
post_setup = [# commands to execute after setup, defaults to []
    "occ app:enable deck",
]

[[volume]] # optional
source = "/tmp/haze-shared"
target = "/shared"
create = true

[[volume]]
source = "/home/me/Downloads"
target = "/Downloads"
read_only = true

[proxy] # optional
address = "haze.example.com" # base domain
https = true # Is the proxy behind an https terminating proxy
listen = "/run/haze/haze.sock" # either a unix socket path
#listen = "127.0.0.1:8080"     # or a socket address

# presets allow for easy usage of commonly used setups
[[preset]]
name = "groupfolders" # name of the preset
apps = ["groupfolders"] # app to enable
commands = ["occ groupfolders:create gf", "occ groupfolders:group 1 admin read write share delete"] # commands to run post-setup
```
