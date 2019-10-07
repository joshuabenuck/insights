// Copyright 2019 - Joshua Benuck
// Licensed under the MIT license
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Results {
    results: Vec<Value>,
}

fn run_query(account_id: &str, api_key: &str, query: &str) -> String {
    let encoded_nrql: String = byte_serialize(query.as_bytes()).collect();
    let client = reqwest::Client::new();
    let url = format!(
        "https://insights-api.newrelic.com/v1/accounts/{}/query?nrql={}",
        account_id, encoded_nrql
    );
    println!("{}", query);
    let mut body = client
        .get(url.as_str())
        .header("Accept", "application/json")
        .header("X-Query-Key", api_key)
        .send()
        .unwrap();
    body.text().unwrap()
}

fn print_results_as_json(results: &str) {
    print!(
        "{}",
        serde_json::to_string_pretty(&serde_json::from_str::<Value>(results).unwrap()).unwrap()
    );
}

fn print_results(results: &str) {
    let parsed = serde_json::from_str::<Results>(results).unwrap();
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

fn get_id_and_key(matches: &ArgMatches) -> (String, String) {
    let account_id = matches.value_of("account_id");
    let api_key = matches.value_of("api_key");
    if account_id == None || api_key == None {
        if account_id != None || api_key != None {
            panic!("Either pass in both account_id and api_key or pull in both from the config.");
        }
        let home_dir = dirs::home_dir();
        if home_dir == None {
            panic!(
                "Unable to find home directory for config. Must provide account_id and api_key!"
            );
        }
        let home_dir = home_dir.unwrap();
        let config_path = format!("{}/.insights.yaml", home_dir.display());
        if !Path::new(config_path.as_str()).exists() {
            panic!(format!(
                "{} does not exist. Must provide account_id and api_key!",
                config_path
            ));
        }
        let file = File::open(config_path).unwrap();
        let config: Config = serde_yaml::from_reader(file).unwrap();
        let account = matches
            .value_of("account")
            .or(config.default.as_ref().map(Deref::deref));
        if account == None {
            panic!("No account specified!");
        }
        let account = account.unwrap();
        let accounts = &(config.accounts).unwrap();
        let account_config = accounts
            .get(account)
            .unwrap_or_else(|| panic!(format!("Unable to find account config for {}!", &account)));
        return (
            account_config.account_id.to_string(),
            account_config.api_key.to_string(),
        );
    }
    return (account_id.unwrap().to_owned(), api_key.unwrap().to_owned());
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
                .arg(
                    Arg::with_name("json")
                        .long("json")
                        .short("j")
                        .help("Print query results as json"),
                )
                .arg(Arg::with_name("nrql").help("The NRQL to run")),
        )
        .subcommand(
            SubCommand::with_name("types")
                .about("Returns the list of known types for the account."),
        )
        .subcommand(
            SubCommand::with_name("attrs")
                .about("Returns the list of known attributes for the event type.")
                .arg(
                    Arg::with_name("type")
                        .required(true)
                        .help("The type for which to display attributes"),
                ),
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
                .arg(Arg::with_name("partial").help("The partial text to complete")),
        )
        .get_matches();
    let (account_id, api_key) = get_id_and_key(&matches);
    if let Some(run) = matches.subcommand_matches("run") {
        let nrql = run.value_of("nrql").unwrap();
        let results = run_query(&account_id, &api_key, nrql);
        match matches.value_of("json") {
            Some(_) => print_results_as_json(results.as_str()),
            None => print_results(results.as_str()),
        }
    }
    if let Some(_types) = matches.subcommand_matches("types") {
        print_results(run_query(&account_id, &api_key, "show event types").as_str());
    }
    if let Some(attrs) = matches.subcommand_matches("attrs") {
        print_results(
            run_query(
                &account_id,
                &api_key,
                format!(
                    "select keyset() from {} since 1 week ago",
                    attrs.value_of("type").unwrap()
                )
                .as_str(),
            )
            .as_str(),
        );
    }
    if let Some(complete) = matches.subcommand_matches("complete") {
        let table = complete.value_of("type").unwrap();
        let column = complete.value_of("attr").unwrap();
        let mut query = format!("select uniques({}) from {}", column, table);
        if let Some(partial) = complete.value_of("partial") {
            query.push_str(format!(" where {} like '{}%'", column, partial).as_str());
        }
        query.push_str(" since 1 week ago");

        print_results(&run_query(&account_id, &api_key, query.as_str()));
    }
}
