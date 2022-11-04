use anyhow::{anyhow, Result};
use itertools::Itertools;
use num_traits::NumCast;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Debug)]
pub struct Line {
    pub label: String,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

pub type Graph = Vec<Line>;
pub type Graphs = HashMap<String, Graph>;

fn fsts<K, V>(hm: &HashMap<K, V>) -> Result<Vec<f64>>
where
    K: NumCast + Ord + Copy + Debug,
{
    hm.iter()
        .sorted_by_key(|x| x.0)
        .map(|x| <f64 as NumCast>::from(*x.0).ok_or(anyhow!("Failed casting {:?} to f64", x.0)))
        .collect()
}
fn snds<K, V>(hm: &HashMap<K, V>) -> Result<Vec<f64>>
where
    K: Ord,
    V: NumCast + Copy + Debug,
{
    hm.iter()
        .sorted_by_key(|x| x.0)
        .map(|x| <f64 as NumCast>::from(*x.1).ok_or(anyhow!("Failed casting {:?} to f64", x.1)))
        .collect()
}

impl Line {
    pub fn try_new<S, K, V>(label: S, hm: &HashMap<K, V>) -> Result<Line>
    where
        K: NumCast + Ord + Copy + Debug,
        V: NumCast + Copy + Debug,
        S: Into<String>,
    {
        Ok(Line {
            label: label.into(),
            x: fsts(hm)?,
            y: snds(hm)?,
        })
    }
}
