use anyhow::Result;
use clap::Parser;
use rtrend::{Client, Country, Keywords, SearchInterest};
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;

pub mod process;

const KEYWORDS: [&str; 2] = ["NixOS", "nix-shell"];

#[derive(Parser, Debug, Clone)]
#[command(version, about, author, long_about = None)]
pub struct Cli {}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TimelineDatum {
    has_data: Vec<bool>,
    is_partial: Option<bool>,
    time: String,
    value: Vec<u64>,
    formatted_axis_time: String,
    formatted_time: String,
    formatted_value: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ResponseDefault {
    timeline_data: Vec<TimelineDatum>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GtrendsResult {
    default: ResponseDefault,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GtrendsData {
    query: Vec<String>,
    result: GtrendsResult,
}

pub async fn run(_args: &Cli) -> Result<()> {
    let result = tokio::task::spawn_blocking(move || {
        let keywords = Keywords::new(Vec::from(KEYWORDS));
        let client = Client::new(keywords, Country::US).build();
        let raw = SearchInterest::new(client).get();
        let res: GtrendsResult = serde_json::from_value(raw).unwrap();
        res
    })
    .await?;

    let output = GtrendsData {
        query: KEYWORDS.map(|x| x.to_string()).to_vec(),
        result,
    };

    println!("{}", to_string_pretty(&output)?);

    Ok(())
}
