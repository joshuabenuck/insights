// Copyright 2019 - Joshua Benuck
// Licensed under the MIT license
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
extern crate failure;
extern crate failure_derive;
use failure::{err_msg, Error, Fail};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_yaml;
use std::cmp::max;
use std::collections::HashMap;
use std::fs::File;
use std::ops::Deref;
use std::path::Path;
use url::form_urlencoded::byte_serialize;

#[derive(Fail, Debug)]
enum InsightsError {
    #[fail(display = "An error occurred.")]
    HomeDirNotFound,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Results {
    results: Vec<Value>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Account {
    account_id: String,
    api_key: String,
    url: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    default: Option<String>,
    accounts: Option<HashMap<String, Account>>,
}

struct QueryResults {
    raw: String,
}

struct Connection {
    account_id: String,
    api_key: String,
    url: String,
}

impl Connection {
    fn from_args(matches: &ArgMatches) -> Result<Connection, Error> {
        let default_url = "https://insights-api.newrelic.com/".to_string();
        let account_id = matches.value_of("account_id");
        let api_key = matches.value_of("api_key");
        let url = matches.value_of("url");
        if account_id == None || api_key == None {
            if account_id != None || api_key != None {
                return Err(err_msg(
                    "Either pass in both account_id and api_key or pull in both from the config.",
                ));
            }
            let home_dir = dirs::home_dir().ok_or(err_msg(
                "Unable to find home directory for config. Must provide account_id and api_key!",
            ))?;
            let config_path = format!("{}/.insights.yaml", home_dir.display());
            if !Path::new(config_path.as_str()).exists() {
                return Err(err_msg(format!(
                    "{} does not exist. Must provide account_id and api_key!",
                    config_path
                )));
            }
            let file = File::open(config_path)?;
            let config: Config = serde_yaml::from_reader(file)?;
            let account = matches
                .value_of("account")
                .or(config.default.as_ref().map(Deref::deref))
                .ok_or(err_msg("No account specified!"))?;
            let accounts = &(config.accounts).unwrap();
            let account_config = accounts.get(account).ok_or(err_msg(format!(
                "Unable to find account config for {}!",
                &account
            )))?;
            let url = account_config
                .url
                .as_ref()
                .unwrap_or(&default_url.to_owned())
                .to_string();
            return Ok(Connection {
                account_id: account_config.account_id.to_string(),
                api_key: account_config.api_key.to_string(),
                url: url.to_string(),
            });
        }

        Ok(Connection {
            account_id: account_id.unwrap().to_owned(),
            api_key: api_key.unwrap().to_owned(),
            url: url.unwrap_or(default_url.as_str()).to_string(),
        })
    }

    fn run_query(&self, query: &str) -> Result<QueryResults, Error> {
        let encoded_nrql: String = byte_serialize(query.as_bytes()).collect();
        let client = reqwest::Client::new();
        let url = format!(
            "{}v1/accounts/{}/query?nrql={}",
            &self.url, &self.account_id, encoded_nrql
        );
        println!("{}", query);
        let mut body = client
            .get(url.as_str())
            .header("Accept", "application/json")
            .header("X-Query-Key", &self.api_key)
            .send()?;
        Ok(QueryResults { raw: body.text()? })
    }
}

#[derive(Debug)]
enum Format {
    Raw,
    JSON,
    CSV,
    Table,
}

impl Format {
    fn from_args(matches: &ArgMatches) -> Format {
        if matches.is_present("json") {
            return Format::JSON;
        }
        if matches.is_present("csv") {
            return Format::CSV;
        }
        if matches.is_present("raw") {
            return Format::Raw;
        }
        if matches.is_present("table") {
            return Format::Table;
        }
        return Format::Table;
    }
}

impl QueryResults {
    fn print(&self, format: Format) -> Result<(), Error> {
        match format {
            Format::Table => self.print_table(),
            Format::JSON => self.print_json(),
            Format::Raw => self.print_raw(),
            Format::CSV => Err(err_msg("Unimplemented Output Format: CSV")),
        }
    }

    fn print_raw(&self) -> Result<(), Error> {
        print!(
            "{}",
            serde_json::to_string_pretty(&serde_json::from_str::<Value>(&self.raw).unwrap())
                .unwrap()
        );
        Ok(())
    }

    fn print_json(&self) -> Result<(), Error> {
        let parsed = serde_json::from_str::<Results>(&self.raw).unwrap();
        // Results are an object with a key of events or eventTypes.
        // Pull the value of whatever is there.
        let props = &parsed.results[0].as_object().unwrap();
        let value = props[props.keys().next().unwrap()].as_array().unwrap();
        println!("{}", serde_json::to_string_pretty(value)?);
        Ok(())
    }

    fn print_table(&self) -> Result<(), Error> {
        let parsed = serde_json::from_str::<Results>(&self.raw).unwrap();
        // Results are an object with a key of events or eventTypes.
        // Pull the value of whatever is there.
        let props = &parsed.results[0].as_object().unwrap();
        let value = props[props.keys().next().unwrap()].as_array().unwrap();
        let mut first: bool = true;
        // Note: This does not properly handle unicode characters!
        let mut widths = Vec::<usize>::new();
        let mut rows = Vec::<Vec<String>>::new();
        for v in value {
            match v {
                Value::String(string_value) => println!("{}", string_value),
                Value::Object(obj_value) => {
                    if first {
                        let mut row = Vec::<String>::new();
                        for key in obj_value.keys() {
                            widths.push(key.len());
                            row.push(key.to_owned());
                        }
                        first = false;
                        rows.push(row);
                    }
                    let row = obj_value
                        .keys()
                        .map(|k| {
                            obj_value[k]
                                .to_string()
                                .trim_matches::<&[char]>(&['"'])
                                .to_owned()
                        })
                        .collect::<Vec<String>>();
                    widths = row
                        .iter()
                        .map(|c| c.len())
                        .zip(widths)
                        .map(|(cw, mw)| max(cw, mw))
                        .collect();
                    rows.push(row);
                }
                _ => println!("Unexpected type in result!"),
            }
        }
        for row in rows {
            for (column, width) in row.iter().zip(&widths) {
                print!("{:<width$} ", column, width = width);
            }
            println!("");
        }
        Ok(())
    }
}

fn process_matches(matches: ArgMatches) -> Result<(), Error> {
    let connection = Connection::from_args(&matches)?;
    if let Some(run) = matches.subcommand_matches("run") {
        let nrql = run.value_of("nrql").unwrap();
        connection.run_query(nrql)?.print(Format::from_args(&run))?;
    }
    if let Some(types) = matches.subcommand_matches("types") {
        connection
            .run_query("show event types")?
            .print(Format::from_args(&types))?;
    }
    if let Some(attrs) = matches.subcommand_matches("attrs") {
        connection
            .run_query(
                format!(
                    "select keyset() from {} since 1 week ago",
                    attrs.value_of("type").unwrap()
                )
                .as_str(),
            )?
            .print(Format::from_args(&attrs))?;
    }
    if let Some(complete) = matches.subcommand_matches("complete") {
        let table = complete.value_of("type").unwrap();
        let column = complete.value_of("attr").unwrap();
        let mut query = format!("select uniques({}) from {}", column, table);
        if let Some(partial) = complete.value_of("partial") {
            query.push_str(format!(" where {} like '{}%'", column, partial).as_str());
        }
        query.push_str(" since 1 week ago");

        connection
            .run_query(query.as_str())?
            .print(Format::from_args(&complete))?;
    }
    Ok(())
}

trait FormattingFlags {
    fn add_formatting_flags(self) -> Self;
}

impl FormattingFlags for App<'_, '_> {
    fn add_formatting_flags(self) -> Self {
        self.arg(
            Arg::with_name("raw")
                .long("raw")
                .help("Return raw query output"),
        )
        .arg(
            Arg::with_name("json")
                .long("json")
                .help("Format output as json"),
        )
        .arg(
            Arg::with_name("csv")
                .long("csv")
                .help("Format output as CSV"),
        )
        .arg(
            Arg::with_name("table")
                .long("table")
                .help("Format output as table (default)"),
        )
    }
}

fn main() {
    let matches = App::new("nrql")
        .version("0.1")
        .author("Joshua Benuck")
        .about("Runs a NRQL query")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::with_name("account")
                .long("account")
                .short("a")
                .takes_value(true)
                .help("Account Config Key"),
        )
        .arg(
            Arg::with_name("account_id")
                .long("account_id")
                .short("i")
                .takes_value(true)
                .help("Account ID"),
        )
        .arg(
            Arg::with_name("api_key")
                .long("api_key")
                .short("k")
                .takes_value(true)
                .help("API Key"),
        )
        .subcommand(
            SubCommand::with_name("run")
                .about("Run an Insights query")
                .arg(Arg::with_name("nrql").help("The NRQL to run"))
                .add_formatting_flags(),
        )
        .subcommand(
            SubCommand::with_name("types")
                .about("Returns the list of known types for the account.")
                .add_formatting_flags(),
        )
        .subcommand(
            SubCommand::with_name("attrs")
                .about("Returns the list of known attributes for the event type.")
                .arg(
                    Arg::with_name("type")
                        .required(true)
                        .help("The type for which to display attributes"),
                )
                .add_formatting_flags(),
        )
        .subcommand(
            SubCommand::with_name("complete")
                .about("Returns a list of valid completions")
                .arg(
                    Arg::with_name("type")
                        .required(true)
                        .help("The event type containing the attribute to complete"),
                )
                .arg(
                    Arg::with_name("attr")
                        .required(true)
                        .help("The attribute to complete"),
                )
                .arg(Arg::with_name("partial").help("The partial text to complete"))
                .add_formatting_flags(),
        )
        .get_matches();
    match process_matches(matches) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1)
        }
    }
}
