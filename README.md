# Haze

Hazy with a chance of clouds.

Easy setup and management of Nextcloud test instances using docker

## What

`haze` provides an easy way to setup Nextcloud test instances with a choice of php version, database server, optional s3 or ldap setup and more.

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
haze start [database] [php-version]
```

Where `database` is one of `sqlite`, `mysql`, `mariadb`, `pgsql` or `oracle` with an optional version (e.g. `pgsql:12`), defaults to `sqlite`.
And `php-version` is one of `8.0`, `8.1`, `8.2`, defaults to `8.1`. `7.3` and `7.4` are still supported but the docker images for those versions aren't being updated anymore so they might be missing some newer features.

Additionally each php version comes with a `-dbg` variant that has php compiled in debug mode and can be used for debugging php itself with gdb.

Additionally, you can use the following options when starting an instance:
 - `s3`: setup an S3 server and configure to Nextcloud to use it as primary storage
 - `<path to app.tar.gz>`: by specifying the path to an app package this package will be extracted into the apps directory of the new instance (overwriting any existing app code). This can be used to quickly test a packaged app. 
 - `ldap`: setup an LDAP server
 - `office`: setup a Nextcloud Office server
 - `onlyoffice` setup an onlyoffice document server
 - `push` setup [client push](https://github.com/nextcloud/notify_push)
 - `smb`: setup a samba server for external storage use
 - `kaspersky`: setup a kaspersky scan engine server in http mode. (Requires [manually setting up the image](https://github.com/icewind1991/kaspersky-docker))
 - `kaspersky-icap`: setup a kaspersky scan engine server in ICAP mode.
 - `clamav-icap`: setup a clam av scanner in ICAP mode.

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

The following commands run against the most recently started instance and allow optionally providing a `match` to select a specific instance by it's name.

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

Runs the provided command with `NEXTCLOUD_URL`, `DATABASE_URL` and `REDIS_URL` environment variables set for the matched instance.

This is indented to run a local [push daemon](https://github.com/nextcloud/notify_push) against an instance. 

## Configuration

Configuration is loaded from `~/.config/haze/haze.toml` and has the following options

```toml
sources_root = "/path/to/sources" # path of the nextcloud sources. required
work_dir = "/path/to/temp/dir" # path to temporary directory. optional, defaults to "/tmp/haze"

[auto_setup] # optional
enabled = false # whether or not to automatically install nextcloud on `haze start`. optional, defaults to false
username = "foo" # username for admin user during auto setup. optional, defaults to "admin"
password = "bar" # password for admin user during auto setup. optional, defaults to "admin"
post_setup = [ # commands to execute after setup, defaults to []
    "occ app:enable deck",
]

[[volume]]
source = "/tmp/haze-shared"
target = "/shared"
create = true

[[volume]]
source = "/home/me/Downloads"
target = "/Downloads"
read_only = true
```