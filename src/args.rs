use crate::cloud::CloudOptions;
use crate::service::Service;
use color_eyre::{Report, Result};
use parse_display::Display;
use std::fmt::Display;
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HazeArgs {
    List {
        filter: Option<String>,
    },
    Start {
        options: CloudOptions,
    },
    Stop {
        filter: Option<String>,
    },
    Test {
        options: CloudOptions,
        args: Vec<String>,
    },
    Exec {
        filter: Option<String>,
        service: Option<ExecService>,
        command: Vec<String>,
    },
    Occ {
        filter: Option<String>,
        command: Vec<String>,
    },
    Db {
        filter: Option<String>,
        root: bool,
    },
    Clean,
    Logs {
        filter: Option<String>,
        service: Option<Service>,
        count: Option<usize>,
    },
    Open {
        filter: Option<String>,
    },
    Fmt {
        path: String,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExecService {
    Db,
}

impl HazeArgs {
    pub fn parse<I, S>(mut args: I) -> Result<HazeArgs>
    where
        S: AsRef<str> + Into<String> + Display + PartialEq<str>,
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
                        })
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
                let options = CloudOptions::parse(&mut args)?;
                if let Some(leftover) = args.next() {
                    return Err(Report::msg(format!("unrecognized option {}", leftover)));
                }
                Ok(HazeArgs::Start { options })
            }
            HazeCommand::Stop => Ok(HazeArgs::Stop { filter }),
            HazeCommand::Test => {
                let mut args = args.peekable();
                let options = CloudOptions::parse(&mut args)?;
                let args = args.map(S::into).collect();
                Ok(HazeArgs::Test { options, args })
            }
            HazeCommand::Exec => {
                let mut args = args.peekable();

                let service = match args.peek() {
                    Some(arg) if arg == "db" => {
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
            HazeCommand::Db => Ok(HazeArgs::Db {
                filter,
                root: args.next().filter(|arg| arg == "root").is_some(),
            }),
            HazeCommand::Clean => Ok(HazeArgs::Clean),
            HazeCommand::Logs => {
                let mut args = args.peekable();
                let service = args
                    .peek()
                    .map(|s| s.as_ref())
                    .map(Service::from_type)
                    .flatten();
                if service.is_some() {
                    let _ = args.next();
                }
                Ok(HazeArgs::Logs {
                    filter,
                    service,
                    count: args.next().map(|arg| arg.as_ref().parse()).transpose()?,
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
        }
    }
}

#[test]
fn test_arg_parse() {
    assert_eq!(
        HazeArgs::parse(vec!["haze"].into_iter()).unwrap(),
        HazeArgs::List { filter: None }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "test"].into_iter()).unwrap(),
        HazeArgs::Test {
            options: Default::default(),
            args: vec![]
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "asdasd"].into_iter()).unwrap(),
        HazeArgs::List {
            filter: Some("asdasd".to_string())
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "asdasd", "db"].into_iter()).unwrap(),
        HazeArgs::Db {
            filter: Some("asdasd".to_string()),
            root: false
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "exec", "foo", "bar"].into_iter()).unwrap(),
        HazeArgs::Exec {
            filter: None,
            service: None,
            command: vec!["foo".to_string(), "bar".to_string()],
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "asdasd", "exec", "foo", "bar"].into_iter()).unwrap(),
        HazeArgs::Exec {
            filter: Some("asdasd".to_string()),
            service: None,
            command: vec!["foo".to_string(), "bar".to_string()],
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "asdasd", "exec", "db", "foo", "bar"].into_iter()).unwrap(),
        HazeArgs::Exec {
            filter: Some("asdasd".to_string()),
            service: Some("db".to_string()),
            command: vec!["foo".to_string(), "bar".to_string()],
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "test", "foo", "bar"].into_iter()).unwrap(),
        HazeArgs::Test {
            options: Default::default(),
            args: vec!["foo".into(), "bar".into()]
        }
    );
}
