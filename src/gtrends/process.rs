use crate::{
    gtrends,
    process::{Graphs, Line},
};
use anyhow::{anyhow, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(version, about, author, long_about = None)]
pub struct Cli {
    // directory where the data has been collected.
    #[clap(long, default_value = ".", value_parser = clap::value_parser!(PathBuf))]
    data: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Data {
    gtrends: HashMap<String, HashMap<u64, f64>>,
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

    fn fsts<V>(hm: &HashMap<u64, V>) -> Result<Vec<f64>> {
        hm.iter()
            .sorted_by_key(|x| x.0)
            .map(|x| <f64 as NumCast>::from(*x.0).ok_or(anyhow!("Failed casting {:?} to f64", x.0)))
            .collect()
    }
    fn snds<V>(hm: &HashMap<u64, V>) -> Result<Vec<f64>>
    where
        V: NumCast + Copy + Debug,
    {
        hm.iter()
            .sorted_by_key(|x| x.0)
            .map(|x| <f64 as NumCast>::from(*x.1).ok_or(anyhow!("Failed casting {:?} to f64", x.1)))
            .collect()
    }

    let graphs: Graphs = HashMap::from([(
        "gtrends".to_owned(),
        data.gtrends
            .iter()
            .map(|(name, gtrend)| {
                Ok(Line {
                    label: name.clone(),
                    x: fsts(gtrend)?,
                    y: snds(gtrend)?,
                })
            })
            .collect::<Result<_>>()?,
    )]);

    println!("{}", serde_json::to_string_pretty(&graphs)?);

    Ok(())
}
