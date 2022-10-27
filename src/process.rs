use super::{gtrends, netlify};
use anyhow::{anyhow, bail, Result};
use chrono::{prelude::DateTime, Utc};
use clap::Parser;
use itertools::Itertools;
use num_traits::cast::NumCast;
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
    gtrends: HashMap<String, HashMap<u64, f64>>,
}

fn process_netlify(data: &mut Data, dir: &PathBuf) -> Result<()> {
    for path in fs::read_dir(dir)
        .map_err(|e| anyhow!("Error listing directory {}: {}", dir.display(), e))?
    {
        let path = path
            .map_err(|e| anyhow!("Error listing directory {}: {}", dir.display(), e))?
            .path();
        let file_content = fs::read_to_string(&path)
            .map_err(|e| anyhow!("Unable to read file {}: {}", path.display(), e))?;
        let json: netlify::MetricsResult = serde_json::from_str(&file_content)
            .map_err(|e| anyhow!("Unable to parse file {}: {}", path.display(), e))?;
        let mut pviews = json.pageviews.unwrap().data;
        // grab this for later, since we're not going to own pviews then
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

    Ok(())
}

fn process_gtrends(data: &mut Data, dir: &PathBuf) -> Result<()> {
    // we have to iterate the directory in order so we have overlap
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| anyhow!("Error listing directory {}: {}", dir.display(), e))?
        .map(|r| {
            r.map(|x| x.path())
                .map_err(|e| anyhow!("Error listing directory {}: {}", dir.display(), e))
        })
        .collect::<Result<_>>()?;
    paths.sort();
    for path in paths {
        let file_content = fs::read_to_string(&path)
            .map_err(|e| anyhow!("Unable to read file {}: {}", path.display(), e))?;
        let json: gtrends::GtrendsData = serde_json::from_str(&file_content)
            .map_err(|e| anyhow!("Unable to parse file {}: {}", path.display(), e))?;

        let mut norm_factor: Option<f64> = None;

        // skip if we're on the fist iteration and are setting the norm
        if data.gtrends.is_empty() {
            norm_factor = Some(1.0);
        }
        // find some overlap and normalize based on that
        else {
            for datum in &json.result.default.timeline_data {
                if datum.is_partial.unwrap_or(false) {
                    continue;
                }
                for (i, name) in json.query.iter().enumerate() {
                    if !datum.has_data[i] {
                        continue;
                    }
                    let time_ms = datum.time.parse::<u64>().unwrap() * 1000;
                    let value = datum.value[i];

                    if let Some(&v) = data.gtrends.get(name).and_then(|x| x.get(&time_ms)) {
                        norm_factor = Some(value as f64 / v as f64);
                        break;
                    }
                }
            }
        }

        let norm_factor = norm_factor.ok_or(anyhow!("Unable to normalize data. There is no overlap between the data in {} and previous days.", path.display()))?;

        for datum in &json.result.default.timeline_data {
            if datum.is_partial.unwrap_or(false) {
                continue;
            }
            for (i, name) in json.query.iter().enumerate() {
                if !datum.has_data[i] {
                    continue;
                }
                let time_ms = datum.time.parse::<u64>().unwrap() * 1000;
                let value = datum.value[i] as f64 * norm_factor;
                let v = data
                    .gtrends
                    .entry(name.to_owned())
                    .or_default()
                    .entry(time_ms)
                    .or_insert(value as f64 * norm_factor);
                // make sure the normalization factor is consistent
                if (*v - value).abs() > f64::EPSILON {
                    bail!("Unable to normalize data: inconsistent normalization factor");
                }
            }
        }
    }

    Ok(())
}

pub async fn run(args: &Cli) -> Result<()> {
    let mut data = Data::default();

    process_netlify(&mut data, &args.dir.join("netlify"))?;
    process_gtrends(&mut data, &args.dir.join("gtrends"))?;

    fn fsts<V>(hm: &HashMap<u64, V>) -> Vec<f64> {
        hm.iter()
            .sorted_by_key(|x| x.0)
            .map(|x| <f64 as NumCast>::from(*x.0).unwrap())
            .collect()
    }
    fn snds<V>(hm: &HashMap<u64, V>) -> Vec<f64>
    where
        V: NumCast + Copy,
    {
        hm.iter()
            .sorted_by_key(|x| x.0)
            .map(|x| <f64 as NumCast>::from(*x.1).unwrap())
            .collect()
    }

    let graphs: Graphs = HashMap::from([
        (
            "pageviews".to_owned(),
            Vec::from([
                Line {
                    label: "Pageviews".to_owned(),
                    x: fsts(&data.pageviews),
                    y: snds(&data.pageviews),
                },
                Line {
                    label: "7 day avg".to_owned(),
                    x: fsts(&data.pageviews_7day),
                    y: snds(&data.pageviews_7day),
                },
            ]),
        ),
        (
            "visitors".to_owned(),
            Vec::from([
                Line {
                    label: "Visitors".to_owned(),
                    x: fsts(&data.visitors),
                    y: snds(&data.visitors),
                },
                Line {
                    label: "7 day avg".to_owned(),
                    x: fsts(&data.visitors_7day),
                    y: snds(&data.visitors_7day),
                },
            ]),
        ),
        (
            "sources".to_owned(),
            data.sources
                .iter()
                .map(|(name, source)| Line {
                    label: if name == "" {
                        "direct".to_owned()
                    } else {
                        name.clone()
                    },
                    x: fsts(source),
                    y: snds(source),
                })
                .collect(),
        ),
        (
            "gtrends".to_owned(),
            data.gtrends
                .iter()
                .map(|(name, gtrend)| Line {
                    label: name.clone(),
                    x: fsts(gtrend),
                    y: snds(gtrend),
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
