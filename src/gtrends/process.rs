use crate::{
    gtrends,
    process::{Graphs, Line, VictoriaMetric, VictoriaMetrics},
};
use anyhow::{anyhow, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(version, about, author, long_about = None)]
pub struct Cli {
    // directory where the data has been collected.
    #[clap(long, default_value = ".", value_parser = clap::value_parser!(PathBuf))]
    data: PathBuf,

    #[clap(long, value_parser = clap::value_parser!(PathBuf))]
    graphs_out: Option<PathBuf>,

    #[clap(long, value_parser = clap::value_parser!(PathBuf))]
    victoriametrics_out: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Data {
    gtrends: BTreeMap<String, BTreeMap<u64, f64>>,
}

pub async fn run(args: &Cli) -> Result<()> {
    let mut data = Data::default();
    let path = &args.data;

    let file_content = fs::read_to_string(&path)
        .map_err(|e| anyhow!("Unable to read file {}: {}", path.display(), e))?;
    let json: gtrends::GtrendsData = serde_json::from_str(&file_content)
        .map_err(|e| anyhow!("Unable to parse file {}: {}", path.display(), e))?;

    for datum in &json.result.default.timeline_data {
        if datum.is_partial.unwrap_or(false) {
            continue;
        }
        for (i, name) in json.query.iter().enumerate() {
            if !datum.has_data[i] {
                continue;
            }
            let time_ms = datum.time.parse::<u64>().unwrap() * 1000;
            let value = datum.value[i] as f64;
            data.gtrends
                .entry(name.to_owned())
                .or_default()
                .entry(time_ms)
                .or_insert(value);
        }
    }

    let graphs: Graphs = HashMap::from([(
        "gtrends".to_owned(),
        data.gtrends
            .iter()
            .map(|(name, gtrend)| Line::try_new(name, gtrend))
            .collect::<Result<_>>()?,
    )]);

    let victoriametrics: VictoriaMetrics = graphs
        .get("gtrends")
        .expect("hard-coded hashmap access of hard-coded entry")
        .into_iter()
        .map(|line| VictoriaMetric::try_new("gtrends", "search_term", line))
        .collect::<Result<_>>()?;

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
