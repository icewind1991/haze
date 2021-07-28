# Haze

Hazy with a chance of clouds.

## Setup

### Requirements

 - Docker

### Config

Create a file `~/.config/haze/haze.toml` with the following options:

```toml
sources_root = "/path/to/nextcloud/sources"
```

See the [configuration section](#configuration) for more options.

## Managing instances

#### Start an instance

```bash
haze start [database] [php-version]
```

Where `database` is one of `sqlite`, `mysql`, `mariadb` or `pgsql` with an optional version (e.g. `pgsql:12`), defaults to `sqlite`.
And `php-version` is one of `7.2`, `7.3`, `7.4`, `8.0`, `7` or `8`, defaults to `8.0`

Additionally, you can use the following options when starting an instance:
 - `s3`: setup an S3 server and configure to Nextcloud to use it as primary storage
 - `<path to app.tar.gz>`: by specifying the path to an app package this package will be extracted into the apps directory of the new instance (overwriting any existing app code). This can be used to quickly test a packaged app. 
 - `ldap`: setup an LDAP server
 - `onlyoffice` setup an onlyoffice document server

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

## Configuration

Configuration is loaded from `~/.config/haze/haze.toml` and has the following options

```toml
sources_root = "/path/to/sources" # path of the nextcloud sources. required
work_dir = "/path/to/temp/dir" # path to temporary directory. optional, defaults to "/tmp/haze"

[auto_setup] # optional
enabled = false # whether or not to automatically install nextcloud on `haze start`. optional, defaults to false
username = "foo" # username for admin user during auto setup. optional, defaults to "admin"
password = "bar" # password for admin user during auto setup. optional, defaults to "admin"

[[volume]]
source = "/tmp/haze-shared"
target = "/shared"
create = true

[[volume]]
source = "/home/me/Downloads"
target = "/Downloads"
read_only = true
```