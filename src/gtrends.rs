use anyhow::Result;
use clap::Parser;
use rtrend::{Client, Country, Keywords, SearchInterest};
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;

const KEYWORDS: [&str; 2] = ["NixOS", "nix-shell"];

#[derive(Parser, Debug, Clone)]
#[command(version, about, author, long_about = None)]
pub struct Cli {}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TimelineDatum {
    pub has_data: Vec<bool>,
    pub is_partial: Option<bool>,
    pub time: String,
    pub value: Vec<u64>,
    pub formatted_axis_time: String,
    pub formatted_time: String,
    pub formatted_value: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResponseDefault {
    pub timeline_data: Vec<TimelineDatum>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GtrendsResult {
    pub default: ResponseDefault,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GtrendsData {
    pub query: Vec<String>,
    pub result: GtrendsResult,
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
