use crate::{
    gtrends,
    process::{Graphs, Line},
};
use anyhow::{anyhow, bail, Result};
use clap::Parser;
use itertools::Itertools;
use num_traits::cast::NumCast;
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
    dir: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Data {
    gtrends: HashMap<String, HashMap<u64, f64>>,
}

pub async fn run(args: &Cli) -> Result<()> {
    let mut data = Data::default();

    // we have to sort the directory before we iterate so we have overlap between subsequent datasets
    let mut paths: Vec<PathBuf> = fs::read_dir(&args.dir)
        .map_err(|e| anyhow!("Error listing directory {}: {}", args.dir.display(), e))?
        .map(|r| {
            r.map(|x| x.path())
                .map_err(|e| anyhow!("Error listing directory {}: {}", args.dir.display(), e))
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
        "grends".to_owned(),
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
