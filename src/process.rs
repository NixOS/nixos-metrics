use anyhow::{anyhow, Result};
use num_traits::NumCast;
use serde::{Deserialize, Serialize};
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
        let (x, y) = hm.into_iter().try_fold(
            (Vec::with_capacity(hm.len()), Vec::with_capacity(hm.len())),
            |(mut xs, mut ys), (x, y)| {
                xs.push(x.to_f64().ok_or(anyhow!("Failed casting {:?} to f64", x))?);
                ys.push(y.to_f64().ok_or(anyhow!("Failed casting {:?} to f64", y))?);
                Ok((xs, ys))
            },
        )?;

        Ok(Line {
            label: label.into(),
            x,
            y,
        })
    }
}
