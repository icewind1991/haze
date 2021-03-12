use color_eyre::{Report, Result};
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HazeArgs {
    pub id: Option<String>,
    pub command: HazeCommand,
    pub options: Vec<String>,
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
                        (Some(sub_or_id.to_string()), HazeCommand::List)
                    }
                }
            }
            None => (None, HazeCommand::List),
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
            command: HazeCommand::List,
            options: Vec::new(),
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "test"].into_iter()).unwrap(),
        HazeArgs {
            id: None,
            command: HazeCommand::Test,
            options: Vec::new(),
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "asdasd"].into_iter()).unwrap(),
        HazeArgs {
            id: Some("asdasd".to_string()),
            command: HazeCommand::List,
            options: Vec::new(),
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "asdasd", "db"].into_iter()).unwrap(),
        HazeArgs {
            id: Some("asdasd".to_string()),
            command: HazeCommand::Db,
            options: Vec::new(),
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "exec", "foo", "bar"].into_iter()).unwrap(),
        HazeArgs {
            id: None,
            command: HazeCommand::Exec,
            options: vec!["foo".to_string(), "bar".to_string()],
        }
    );
    assert_eq!(
        HazeArgs::parse(vec!["haze", "asdasd", "exec", "foo", "bar"].into_iter()).unwrap(),
        HazeArgs {
            id: Some("asdasd".to_string()),
            command: HazeCommand::Exec,
            options: vec!["foo".to_string(), "bar".to_string()],
        }
    );
}
