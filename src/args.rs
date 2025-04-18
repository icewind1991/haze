use crate::cloud::CloudOptions;
use crate::config::Preset;
use crate::service::{Service, ServiceTrait};
use miette::{IntoDiagnostic, Report, Result};
use parse_display::Display;
use std::fmt::Display;
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HazeArgs {
    /// List all instances
    List { filter: Option<String> },
    /// Start a new instance
    Start { options: CloudOptions },
    /// Stop an instance
    Stop { filter: Option<String> },
    /// Run tests in a new instance
    Test {
        options: CloudOptions,
        args: Vec<String>,
    },
    /// Run a command in an instance
    Exec {
        filter: Option<String>,
        service: Option<ExecService>,
        command: Vec<String>,
    },
    /// Run an occ command in an instance
    Occ {
        filter: Option<String>,
        command: Vec<String>,
    },
    /// Connect to the database of an instance
    Db {
        filter: Option<String>,
        root: bool,
        command: Vec<String>,
    },
    /// Remove all non-pinned instances
    Clean,
    /// View the logs from an instance or service
    Logs {
        filter: Option<String>,
        follow: bool,
        service: Option<LogService>,
        count: Option<usize>,
    },
    /// Open an instance in the browser
    Open { filter: Option<String> },
    /// Run code formatting from a new instance
    Fmt { path: String },
    /// Run integration tests in a new instance
    Integration {
        options: CloudOptions,
        args: Vec<String>,
    },
    /// Start a shell in an empirical instance
    Shell {
        options: CloudOptions,
        command: Vec<String>,
    },
    /// Pin an instance
    Pin { filter: Option<String> },
    /// Unpin an instance
    Unpin { filter: Option<String> },
    /// Start the proxy
    Proxy,
    /// Checkout a branch in all apps
    Checkout { branch: String },
    Env {
        filter: Option<String>,
        command: String,
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LogService {
    Service(Service),
    Database,
}

impl LogService {
    pub fn from_type(presets: &[Preset], ty: &str) -> Option<Self> {
        if ty == "db" {
            return Some(LogService::Database);
        }
        Some(LogService::Service(
            Service::from_type(presets, ty)?.into_iter().next()?,
        ))
    }

    pub fn container_name(&self, cloud_id: &str) -> Option<String> {
        match self {
            LogService::Database => Some(format!("{}-db", cloud_id)),
            LogService::Service(service) => service.container_name(cloud_id),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExecService {
    Db,
}

impl HazeArgs {
    pub fn parse<I, S>(presets: &[Preset], mut args: I) -> Result<HazeArgs>
    where
        S: AsRef<str> + Into<String> + Display,
        I: Iterator<Item = S>,
    {
        let _bin = args.next();
        let command_or_filter = match args.next() {
            Some(s) => s,
            None => return Ok(HazeArgs::List { filter: None }),
        };
        let (cmd, filter) = match HazeCommand::from_str(command_or_filter.as_ref()) {
            Ok(cmd) => (cmd, None),
            Err(_) => {
                let cmd = match args.next() {
                    Some(cmd) => HazeCommand::from_str(cmd.as_ref())?,
                    None => {
                        return Ok(HazeArgs::List {
                            filter: Some(command_or_filter.into()),
                        });
                    }
                };
                if !cmd.allows_filter() {
                    return Err(Report::msg(format!(
                        "{} doesn't allow specifying a filter",
                        cmd
                    )));
                }
                (cmd, Some(command_or_filter.into()))
            }
        };

        match cmd {
            HazeCommand::List => Ok(HazeArgs::List {
                filter: filter.or_else(|| args.next().map(S::into)),
            }),
            HazeCommand::Start => {
                let mut args = args.peekable();
                let options = CloudOptions::parse(presets, &mut args)?;
                if let Some(leftover) = args.next() {
                    return Err(Report::msg(format!("unrecognized option {}", leftover)));
                }
                Ok(HazeArgs::Start { options })
            }
            HazeCommand::Stop => Ok(HazeArgs::Stop { filter }),
            HazeCommand::Test => {
                let mut args = args.peekable();
                let options = CloudOptions::parse(presets, &mut args)?;
                let args = args.map(S::into).collect();
                Ok(HazeArgs::Test { options, args })
            }
            HazeCommand::Integration => {
                let mut args = args.peekable();
                let options = CloudOptions::parse(presets, &mut args)?;
                let args = args.map(S::into).collect();
                Ok(HazeArgs::Integration { options, args })
            }
            HazeCommand::Exec => {
                let mut args = args.peekable();

                let service = match args.peek() {
                    Some(arg) if arg.as_ref() == "db" => {
                        args.next();
                        Some(ExecService::Db)
                    }
                    _ => None,
                };

                let command = args.map(S::into).collect();
                Ok(HazeArgs::Exec {
                    filter,
                    service,
                    command,
                })
            }
            HazeCommand::Occ => Ok(HazeArgs::Occ {
                filter,
                command: args.map(S::into).collect(),
            }),
            HazeCommand::Db => {
                let mut args = args.peekable();
                let root = if let Some(first) = args.peek() {
                    let root = first.as_ref() == "root";
                    if root {
                        let _ = args.next();
                    }
                    root
                } else {
                    false
                };
                let command = args.map(S::into).collect();
                Ok(HazeArgs::Db {
                    filter,
                    root,
                    command,
                })
            }
            HazeCommand::Clean => Ok(HazeArgs::Clean),
            HazeCommand::Logs => {
                let mut args = args.peekable();
                let follow = args.next_if(|arg| arg.as_ref() == "-f").is_some();
                let service = args
                    .next_if(|arg| LogService::from_type(presets, arg.as_ref()).is_some())
                    .and_then(|arg| LogService::from_type(presets, arg.as_ref()));
                Ok(HazeArgs::Logs {
                    filter,
                    follow,
                    service,
                    count: args
                        .next()
                        .map(|arg| arg.as_ref().parse())
                        .transpose()
                        .into_diagnostic()?,
                })
            }
            HazeCommand::Open => Ok(HazeArgs::Open { filter }),
            HazeCommand::Fmt => {
                let path = args
                    .next()
                    .map(S::into)
                    .ok_or_else(|| Report::msg("No path provided"))?;
                Ok(HazeArgs::Fmt { path })
            }
            HazeCommand::Shell => {
                let mut args = args.peekable();
                let options = CloudOptions::parse(presets, &mut args)?;
                let command = args.map(S::into).collect();
                Ok(HazeArgs::Shell { options, command })
            }
            HazeCommand::Pin => Ok(HazeArgs::Pin { filter }),
            HazeCommand::Unpin => Ok(HazeArgs::Unpin { filter }),
            HazeCommand::Proxy => Ok(HazeArgs::Proxy),
            HazeCommand::Checkout => {
                let branch = args
                    .next()
                    .map(S::into)
                    .ok_or_else(|| Report::msg("No branch provided"))?;
                Ok(HazeArgs::Checkout { branch })
            }
            HazeCommand::Env => {
                let mut args = args.map(S::into);
                let command = args
                    .next()
                    .ok_or_else(|| Report::msg("No command provided"))?;
                Ok(HazeArgs::Env {
                    filter,
                    command,
                    args: args.collect(),
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Display)]
pub enum HazeCommand {
    List,
    Start,
    Stop,
    Test,
    Exec,
    Occ,
    Db,
    Clean,
    Logs,
    Open,
    Fmt,
    Integration,
    Shell,
    Pin,
    Unpin,
    Proxy,
    Checkout,
    Env,
}

impl FromStr for HazeCommand {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "list" => Ok(HazeCommand::List),
            "start" => Ok(HazeCommand::Start),
            "stop" => Ok(HazeCommand::Stop),
            "test" => Ok(HazeCommand::Test),
            "exec" => Ok(HazeCommand::Exec),
            "occ" => Ok(HazeCommand::Occ),
            "db" => Ok(HazeCommand::Db),
            "clean" => Ok(HazeCommand::Clean),
            "logs" => Ok(HazeCommand::Logs),
            "open" => Ok(HazeCommand::Open),
            "fmt" => Ok(HazeCommand::Fmt),
            "format" => Ok(HazeCommand::Fmt),
            "integration" => Ok(HazeCommand::Integration),
            "shell" => Ok(HazeCommand::Shell),
            "pin" => Ok(HazeCommand::Pin),
            "unpin" => Ok(HazeCommand::Unpin),
            "proxy" => Ok(HazeCommand::Proxy),
            "checkout" => Ok(HazeCommand::Checkout),
            "env" => Ok(HazeCommand::Env),
            _ => Err(Report::msg(format!("Unknown command: {}", s))),
        }
    }
}

impl HazeCommand {
    pub fn allows_filter(&self) -> bool {
        match self {
            HazeCommand::List => true,
            HazeCommand::Start => false,
            HazeCommand::Stop => true,
            HazeCommand::Test => false,
            HazeCommand::Exec => true,
            HazeCommand::Occ => true,
            HazeCommand::Db => true,
            HazeCommand::Clean => false,
            HazeCommand::Logs => true,
            HazeCommand::Open => true,
            HazeCommand::Fmt => false,
            HazeCommand::Integration => false,
            HazeCommand::Shell => false,
            HazeCommand::Pin => true,
            HazeCommand::Unpin => true,
            HazeCommand::Proxy => false,
            HazeCommand::Checkout => false,
            HazeCommand::Env => true,
        }
    }
}

#[test]
fn test_arg_parse() {
    assert_eq!(
        HazeArgs::parse(&[], vec!["haze"].into_iter()).unwrap(),
        HazeArgs::List { filter: None }
    );
    assert_eq!(
        HazeArgs::parse(&[], vec!["haze", "test"].into_iter()).unwrap(),
        HazeArgs::Test {
            options: Default::default(),
            args: vec![]
        }
    );
    assert_eq!(
        HazeArgs::parse(&[], vec!["haze", "asdasd"].into_iter()).unwrap(),
        HazeArgs::List {
            filter: Some("asdasd".to_string())
        }
    );
    assert_eq!(
        HazeArgs::parse(&[], vec!["haze", "asdasd", "db"].into_iter()).unwrap(),
        HazeArgs::Db {
            filter: Some("asdasd".to_string()),
            root: false,
            command: Vec::new()
        }
    );
    assert_eq!(
        HazeArgs::parse(&[], vec!["haze", "asdasd", "db", "root"].into_iter()).unwrap(),
        HazeArgs::Db {
            filter: Some("asdasd".to_string()),
            root: true,
            command: Vec::new()
        }
    );
    assert_eq!(
        HazeArgs::parse(&[], vec!["haze", "asdasd", "db", "select", "1"].into_iter()).unwrap(),
        HazeArgs::Db {
            filter: Some("asdasd".to_string()),
            root: false,
            command: vec!["select".to_string(), "1".to_string()]
        }
    );
    assert_eq!(
        HazeArgs::parse(
            &[],
            vec!["haze", "asdasd", "db", "root", "select 1"].into_iter()
        )
        .unwrap(),
        HazeArgs::Db {
            filter: Some("asdasd".to_string()),
            root: true,
            command: vec!["select 1".to_string()]
        }
    );
    assert_eq!(
        HazeArgs::parse(&[], vec!["haze", "exec", "foo", "bar"].into_iter()).unwrap(),
        HazeArgs::Exec {
            filter: None,
            service: None,
            command: vec!["foo".to_string(), "bar".to_string()],
        }
    );
    assert_eq!(
        HazeArgs::parse(
            &[],
            vec!["haze", "asdasd", "exec", "foo", "bar"].into_iter()
        )
        .unwrap(),
        HazeArgs::Exec {
            filter: Some("asdasd".to_string()),
            service: None,
            command: vec!["foo".to_string(), "bar".to_string()],
        }
    );
    assert_eq!(
        HazeArgs::parse(
            &[],
            vec!["haze", "asdasd", "exec", "db", "foo", "bar"].into_iter()
        )
        .unwrap(),
        HazeArgs::Exec {
            filter: Some("asdasd".to_string()),
            service: Some(ExecService::Db),
            command: vec!["foo".to_string(), "bar".to_string()],
        }
    );
    assert_eq!(
        HazeArgs::parse(&[], vec!["haze", "test", "foo", "bar"].into_iter()).unwrap(),
        HazeArgs::Test {
            options: Default::default(),
            args: vec!["foo".into(), "bar".into()]
        }
    );
    assert_eq!(
        HazeArgs::parse(&[], vec!["haze", "logs", "-f", "smb"].into_iter()).unwrap(),
        HazeArgs::Logs {
            filter: None,
            follow: true,
            service: Some(LogService::from_type(&[], "smb").unwrap()),
            count: None,
        }
    );
    assert_eq!(
        HazeArgs::parse(
            &[],
            vec!["haze", "asdasd", "logs", "smb", "123"].into_iter()
        )
        .unwrap(),
        HazeArgs::Logs {
            filter: Some("asdasd".to_string()),
            follow: false,
            service: Some(LogService::from_type(&[], "smb").unwrap()),
            count: Some(123),
        }
    );
}
