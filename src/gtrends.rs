use anyhow::Result;
use chrono::{NaiveDate, TimeZone, Utc};
use clap::Parser;
use rtrend::{Client, Country, Keywords, SearchInterest};
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;

pub mod process;

const KEYWORDS: [&str; 3] = ["NixOS", "nix-shell", "nixpkgs"];

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
    let result = tokio::task::spawn_blocking(move || -> Result<_> {
        let keywords = Keywords::new(KEYWORDS.to_vec());
        let country = Country::ALL;
        // start at 2012, because before that the data gets weirdly high. maybe "nixos" meant something else?
        let start: NaiveDate = Utc
            .with_ymd_and_hms(2012, 1, 1, 0, 0, 0)
            .unwrap()
            .date_naive();
        let end: NaiveDate = Utc::now().date_naive();
        let client = Client::new(keywords, country).with_date(start, end).build();
        let raw = SearchInterest::new(client).get();
        let res: GtrendsResult = serde_json::from_value(raw)?;
        Ok(res)
    })
    .await??;

    let output = GtrendsData {
        query: KEYWORDS.map(|x| x.to_string()).to_vec(),
        result,
    };

    println!("{}", to_string_pretty(&output)?);

    Ok(())
}
