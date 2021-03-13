# Haze

Hazy with a chance of clouds.

## Setup

### Requirements

 - Docker

### Config

Create a file `~/.config/haze/haze.toml` with the following options:

```toml
sources_root = "/path/to/nextcloud/sources"
work_dir = "/path/to/temp/folder" # optional, defaults to /tmp/haze
```

## Managing instances

#### Start an instance

```bash
haze start [database]
```

Where database is one of `sqlite`, `mysql`, `mariadb` or `pgsql` with an optional version (e.g. `pgsql:12`), defaults to `sqlite`.

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

#### Show the logs of an instance

```bash
haze [match] logs
```

#### Stop an instance

```bash
haze [match] stop
```
