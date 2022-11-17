use anyhow::{anyhow, Result};
use num_traits::{NumCast, ToPrimitive};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Debug)]
pub struct Line {
    pub label: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

pub type Graph = Vec<Line>;
pub type Graphs = HashMap<String, Graph>;

impl Line {
    pub fn try_new<K, V>(label: impl Into<String>, hm: &BTreeMap<K, V>) -> Result<Line>
    where
        K: NumCast + Ord + Copy + Debug,
        V: NumCast + Copy + Debug,
    {
        let (x, y) = hm
            .into_iter()
            .map(|(x, y)| {
                let x = x.to_f64().ok_or(anyhow!("Failed casting {:?} to f64", x))?;
                let y = y.to_f64().ok_or(anyhow!("Failed casting {:?} to f64", y))?;
                Ok((x, y))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .unzip();

        Ok(Line {
            label: label.into(),
            x,
            y,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VictoriaMetric {
    // `metric` must have `__name__` field. Everything else are labels
    // TODO: Should we use a type that enforces this, and manually impl Serialize, Deserialize?
    pub metric: serde_json::Value,
    pub values: Vec<f64>,
    pub timestamps: Vec<u64>,
}
pub type VictoriaMetrics = Vec<VictoriaMetric>;

impl VictoriaMetric {
    pub fn try_new(
        name: impl Into<String>,
        label: impl Into<String>,
        line: &Line,
    ) -> Result<VictoriaMetric> {
        Ok(VictoriaMetric {
            // TODO: there's got to be a cleaner way to write this
            metric: match label.into().as_str() {
                "" => json!({"__name__": name.into()}),
                label => json!({
                    "__name__": name.into(),
                    label.to_owned(): line.label,
                }),
            },
            values: line.y.clone(),
            timestamps: line
                .x
                .clone()
                .into_iter()
                .map(|x| x.to_u64().ok_or(anyhow!("Filaed casting {:?} to u64", x)))
                .collect::<Result<_>>()?,
        })
    }
}
