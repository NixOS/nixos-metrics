use super::netlify;
use anyhow::Result;
use chrono::{prelude::DateTime, Utc};
use clap::Parser;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::exit;
use std::time::{Duration, UNIX_EPOCH};

const MS_PER_DAY: u64 = 1000 * 60 * 60 * 24;

#[derive(Parser, Debug, Clone)]
#[command(version, about, author, long_about = None)]
pub struct Cli {
    // directory where the data has been collected.
    #[clap(long, default_value = ".", value_parser = clap::value_parser!(PathBuf))]
    dir: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
struct Line {
    label: String,
    x: Vec<f64>,
    y: Vec<f64>,
}

type Graph = Vec<Line>;
type Graphs = HashMap<String, Graph>;

#[derive(Serialize, Deserialize, Debug)]
struct Data {
    pageviews: HashMap<u64, u64>,
    pageviews_7day: HashMap<u64, f64>,
    visitors: HashMap<u64, u64>,
    visitors_7day: HashMap<u64, f64>,
    sources: HashMap<String, HashMap<u64, u64>>,
}

impl Data {
    fn new() -> Self {
        Self {
            pageviews: HashMap::new(),
            pageviews_7day: HashMap::new(),
            visitors: HashMap::new(),
            visitors_7day: HashMap::new(),
            sources: HashMap::new(),
        }
    }
}

pub async fn run(args: &Cli) -> Result<()> {
    let mut data = Data::new();

    for path in fs::read_dir(&args.dir)
        .unwrap_or_else(|e| panic!("Error listing directory {}: {}", args.dir.display(), e))
    {
        let path = path
            .unwrap_or_else(|e| panic!("Error listing directory {}: {}", args.dir.display(), e))
            .path();
        let file_content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Unable to read file {}: {}", path.display(), e));
        let json: netlify::MetricsResult = serde_json::from_str(&file_content)
            .unwrap_or_else(|e| panic!("Unable to parse file {}: {}", path.display(), e));
        let pviews = &json.pageviews.as_ref().unwrap().data;
        for &(tstamp, datum) in pviews[..pviews.len() - 1].iter() {
            if let Some(v) = data.pageviews.insert(tstamp, datum) {
                if v != datum {
                    println!(
                        "data mistmatch on {} (pageviews): {} from before and {} found in file {}",
                        to_date(tstamp),
                        v,
                        datum,
                        path.display()
                    );
                    exit(1)
                }
            }
        }

        let visitors = &json.visitors.as_ref().unwrap().data;
        for &(tstamp, datum) in visitors[..visitors.len() - 1].iter() {
            if let Some(v) = data.visitors.insert(tstamp, datum) {
                if v != datum {
                    println!(
                        "data mistmatch on {} (visitors): {} from before and {} found in file {}",
                        to_date(tstamp),
                        v,
                        datum,
                        path.display()
                    );
                    exit(1)
                }
            }
        }
        let current_date = pviews
            .last()
            .unwrap_or_else(|| panic!("Error empty pageviews in file {}", path.display()))
            .0;
        let sources = &json.sources.as_ref().unwrap().data;
        for source in sources[..sources.len() - 1].iter() {
            data.sources
                .entry(source.path.clone())
                .or_insert(HashMap::new())
                .insert(current_date, source.count);
        }
    }

    data.pageviews_7day = avg_7day(&data.pageviews);
    data.visitors_7day = avg_7day(&data.visitors);

    let graphs: Graphs = HashMap::from([
        (
            String::from("pageviews"),
            Vec::from([
                Line {
                    label: String::from("Pageviews"),
                    x: data
                        .pageviews
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .map(|(a, _)| *a as f64)
                        .collect(),
                    y: data
                        .pageviews
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .map(|(_, b)| *b as f64)
                        .collect(),
                },
                Line {
                    label: String::from("7 day avg"),
                    x: data
                        .pageviews_7day
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .map(|(a, _)| *a as f64)
                        .collect(),
                    y: data
                        .pageviews_7day
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .map(|(_, b)| *b as f64)
                        .collect(),
                },
            ]),
        ),
        (
            String::from("visitors"),
            Vec::from([
                Line {
                    label: String::from("Visitors"),
                    x: data
                        .visitors
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .map(|(a, _)| *a as f64)
                        .collect(),
                    y: data
                        .visitors
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .map(|(_, b)| *b as f64)
                        .collect(),
                },
                Line {
                    label: String::from("7 day avg"),
                    x: data
                        .visitors_7day
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .map(|(a, _)| *a as f64)
                        .collect(),
                    y: data
                        .visitors_7day
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .map(|(_, b)| *b as f64)
                        .collect(),
                },
            ]),
        ),
        (
            String::from("sources"),
            data.sources
                .iter()
                .map(|(name, source)| Line {
                    label: name.clone(),
                    x: source
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .map(|(a, _)| *a as f64)
                        .collect(),
                    y: source
                        .iter()
                        .sorted_by_key(|x| x.0)
                        .map(|(_, b)| *b as f64)
                        .collect(),
                })
                .collect(),
        ),
    ]);
    println!("{}", serde_json::to_string_pretty(&graphs)?);

    Ok(())
}

fn to_date(ms: u64) -> String {
    DateTime::<Utc>::from(UNIX_EPOCH + Duration::from_millis(ms))
        .format("%Y-%m-%d")
        .to_string()
}

fn avg_7day(data: &HashMap<u64, u64>) -> HashMap<u64, f64> {
    let mut avg: HashMap<u64, (f64, u8)> = HashMap::new();

    for (date, datum) in data {
        for i in 0..7 {
            let date = *date + (i * MS_PER_DAY);

            let (ref mut avg, ref mut days) = avg.entry(date).or_insert((0.0, 0));

            *avg += *datum as f64 / 7.0;
            *days += 1
        }
    }
    avg.iter()
        .filter_map(|(date, (avg, days))| (*days == 7).then_some((*date, *avg)))
        .collect()
}
