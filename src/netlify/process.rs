use crate::{
    netlify,
    process::{Graphs, Line, VictoriaMetric, VictoriaMetrics},
};
use anyhow::{anyhow, bail, Result};
use chrono::{prelude::DateTime, Utc};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

const MS_PER_DAY: u64 = 1000 * 60 * 60 * 24;

#[derive(Parser, Debug, Clone)]
#[command(version, about, author, long_about = None)]
pub struct Cli {
    // directory where the data has been collected.
    #[clap(long, default_value = ".", value_parser = clap::value_parser!(PathBuf))]
    dir: PathBuf,

    #[clap(long, value_parser = clap::value_parser!(PathBuf))]
    graphs_out: Option<PathBuf>,

    #[clap(long, value_parser = clap::value_parser!(PathBuf))]
    victoriametrics_out: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Data {
    pageviews: BTreeMap<u64, u64>,
    pageviews_7day: BTreeMap<u64, f64>,
    visitors: BTreeMap<u64, u64>,
    visitors_7day: BTreeMap<u64, f64>,
    sources: HashMap<String, BTreeMap<u64, u64>>,
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
        let mut pviews = json
            .pageviews
            .ok_or(anyhow!("No pageviews data in {}", path.display()))?
            .data;
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

        let mut visitors = json
            .visitors
            .ok_or(anyhow!("No visitors data in {}", path.display()))?
            .data;
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

        let mut sources = json
            .sources
            .ok_or(anyhow!("No sources data in {}", path.display()))?
            .data;
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
                Line::try_new("Pageviews", &data.pageviews)?,
                Line::try_new("7 day avg", &data.pageviews_7day)?,
            ]),
        ),
        (
            "visitors".to_owned(),
            Vec::from([
                Line::try_new("Visitors", &data.visitors)?,
                Line::try_new("7 day avg", &data.visitors_7day)?,
            ]),
        ),
        (
            "sources".to_owned(),
            data.sources
                .iter()
                .map(|(name, source)| {
                    Line::try_new(
                        if name == "" {
                            "direct".to_owned()
                        } else {
                            name.clone()
                        },
                        source,
                    )
                })
                .collect::<Result<_>>()?,
        ),
    ]);

    let mut victoriametrics: VictoriaMetrics = vec![
        VictoriaMetric::try_new(
            "netlify.pageviews",
            "",
            &graphs
                .get("pageviews")
                .expect("hard-coded hashmap access of hard-coded entry")[0],
        )?,
        VictoriaMetric::try_new(
            "netlify.visitors",
            "",
            &graphs
                .get("visitors")
                .expect("hard-coded hashmap access of hard-coded entry")[0],
        )?,
    ];

    for source in graphs
        .get("sources")
        .expect("hard-coded hashmap access of hard-coded entry")
    {
        victoriametrics.push(VictoriaMetric::try_new(
            "netlify.sources",
            "source",
            &source,
        )?);
    }

    if let Some(graphs_out) = &args.graphs_out {
        let mut graphs_out = fs::File::create(graphs_out)?;

        writeln!(
            &mut graphs_out,
            "{}",
            serde_json::to_string_pretty(&graphs)?
        )?;
    }

    if let Some(victoriametrics_out) = &args.victoriametrics_out {
        let mut victoriametrics_out = fs::File::create(victoriametrics_out)?;
        for victoriametric in victoriametrics {
            serde_json::to_writer(&mut victoriametrics_out, &victoriametric)?;
            // above doesn't end in a newline
            writeln!(&mut victoriametrics_out, "")?;
        }
    }

    Ok(())
}

fn to_date(ms: u64) -> String {
    DateTime::<Utc>::from(UNIX_EPOCH + Duration::from_millis(ms))
        .format("%Y-%m-%d")
        .to_string()
}

fn avg_7day(data: &BTreeMap<u64, u64>) -> BTreeMap<u64, f64> {
    let mut avgs = BTreeMap::<u64, f64>::default();
    let mut days = BTreeMap::<u64, u8>::default();

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
