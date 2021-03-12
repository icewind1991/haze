use color_eyre::{eyre::WrapErr, Report, Result};
use std::env::Args;
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HazeArgs {
    id: Option<String>,
    command: Command,
    options: Vec<String>,
}

impl HazeArgs {
    pub fn parse<I, S>(mut args: I) -> Result<HazeArgs>
    where
        S: AsRef<str> + ToString,
        I: Iterator<Item = S>,
    {
        let _bin = args.next().unwrap();
        let (id, command) = match args.next() {
            Some(sub_or_id) => {
                if let Ok(command) = sub_or_id.as_ref().parse() {
                    (None, command)
                } else {
                    if let Some(sub) = args.next() {
                        (Some(sub_or_id.to_string()), sub.as_ref().parse()?)
                    } else {
                        (Some(sub_or_id.to_string()), Command::List)
                    }
                }
            }
            None => (None, Command::List),
        };
        let options = args.map(|s| s.to_string()).collect();
        Ok(HazeArgs {
            id,
            command,
            options,
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Command {
    List,
    Start,
    Stop,
    Test,
    Exec,
    Occ,
    Db,
}

impl FromStr for Command {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "list" => Ok(Command::List),
            "start" => Ok(Command::Start),
            "stop" => Ok(Command::Stop),
            "test" => Ok(Command::Test),
            "exec" => Ok(Command::Exec),
            "occ" => Ok(Command::Occ),
            "db" => Ok(Command::Db),
            _ => Err(Report::msg(format!("Unknown command: {}", s))),
        }
    }
}

#[test]
fn test_arg_parse() {
    assert_eq!(
        HazeArgs::parse(vec!["haze"].into_iter()).unwrap(),
        HazeArgs {
            id: None,
            command: Command::List,
            options: Vec::new(),
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "test"].into_iter()).unwrap(),
        HazeArgs {
            id: None,
            command: Command::Test,
            options: Vec::new(),
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "asdasd"].into_iter()).unwrap(),
        HazeArgs {
            id: Some("asdasd".to_string()),
            command: Command::List,
            options: Vec::new(),
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "asdasd", "db"].into_iter()).unwrap(),
        HazeArgs {
            id: Some("asdasd".to_string()),
            command: Command::Db,
            options: Vec::new(),
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "exec", "foo", "bar"].into_iter()).unwrap(),
        HazeArgs {
            id: None,
            command: Command::Exec,
            options: vec!["foo".to_string(), "bar".to_string()],
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "asdasd", "exec", "foo", "bar"].into_iter()).unwrap(),
        HazeArgs {
            id: Some("asdasd".to_string()),
            command: Command::Exec,
            options: vec!["foo".to_string(), "bar".to_string()],
        }
    );
}
