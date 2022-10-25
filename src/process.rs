use super::netlify;
use anyhow::{anyhow, bail, Result};
use chrono::{prelude::DateTime, Utc};
use clap::Parser;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
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

#[derive(Serialize, Deserialize, Debug, Default)]
struct Data {
    pageviews: HashMap<u64, u64>,
    pageviews_7day: HashMap<u64, f64>,
    visitors: HashMap<u64, u64>,
    visitors_7day: HashMap<u64, f64>,
    sources: HashMap<String, HashMap<u64, u64>>,
}

pub async fn run(args: &Cli) -> Result<()> {
    let mut data = Data::default();

    for path in fs::read_dir(&args.dir)
        .map_err(|e| anyhow!("Error listing directory {}: {}", args.dir.display(), e))?
    {
        let path = path
            .map_err(|e| anyhow!("Error listing directory {}: {}", args.dir.display(), e))?
            .path();
        let file_content = fs::read_to_string(&path)
            .map_err(|e| anyhow!("Unable to read file {}: {}", path.display(), e))?;
        let json: netlify::MetricsResult = serde_json::from_str(&file_content)
            .map_err(|e| anyhow!("Unable to parse file {}: {}", path.display(), e))?;
        let mut pviews = json.pageviews.unwrap().data;
        let current_date = pviews
            .last()
            .ok_or_else(|| anyhow!("Error empty pageviews in file {}", path.display()))?
            .0;
        pviews.truncate(pviews.len() - 1);
        for (tstamp, datum) in pviews {
            let v = *data.pageviews.entry(tstamp).or_insert(datum);
            if v != datum {
                bail!(
                    "data mistmatch on {} (pageviews): {} from before and {} found in file {}",
                    to_date(tstamp),
                    v,
                    datum,
                    path.display()
                );
            }
        }

        let mut visitors = json.visitors.unwrap().data;
        visitors.truncate(visitors.len() - 1);
        for (tstamp, datum) in visitors {
            let v = *data.visitors.entry(tstamp).or_insert(datum);
            if v != datum {
                bail!(
                    "data mistmatch on {} (visitors): {} from before and {} found in file {}",
                    to_date(tstamp),
                    v,
                    datum,
                    path.display()
                );
            }
        }

        let mut sources = json.sources.unwrap().data;
        sources.truncate(sources.len() - 1);
        for source in sources {
            data.sources
                .entry(source.path)
                .or_default()
                .insert(current_date, source.count);
        }
    }

    data.pageviews_7day = avg_7day(&data.pageviews);
    data.visitors_7day = avg_7day(&data.visitors);

    let graphs: Graphs = HashMap::from([
        (
            "pageviews".to_owned(),
            Vec::from([
                Line {
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
                    label: "Pageviews".to_owned(),
                },
                Line {
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
                    label: "7 day avg".to_owned(),
                },
            ]),
        ),
        (
            "visitors".to_owned(),
            Vec::from([
                Line {
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
                    label: "Visitors".to_owned(),
                },
                Line {
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
                    label: "7 day avg".to_owned(),
                },
            ]),
        ),
        (
            "sources".to_owned(),
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
    let mut avgs = HashMap::<u64, f64>::default();
    let mut days = HashMap::<u64, u8>::default();

    // average over the 7 days after a date
    for (&date, &datum) in data {
        for i in 0..7 {
            let date = date + (i * MS_PER_DAY);

            let avg = avgs.entry(date).or_insert(0.0);
            let n = days.entry(date).or_insert(0);

            *avg += datum as f64 / 7.0;
            *n += 1
        }
    }

    // only keep the ones we have a full 7 days of data for
    for (date, n) in days {
        if n != 7 {
            avgs.remove(&date);
        }
    }
    avgs
}
