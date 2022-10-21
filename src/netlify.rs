use anyhow::{anyhow, Context, Result};
use chrono::{Duration, Local};
use clap::Parser;
use futures::stream::iter as stream_iter;
use futures::StreamExt;
use log::{error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string_pretty};
use std::fmt::{Display, Formatter, Result as FmtResult};

fn parse_days(src: &str) -> Result<i64> {
    let days = src.parse()?;
    if days > 30 {
        return Err(anyhow!(
            "Days can be set at maximum to 30, but are set to {}.",
            src
        ));
    }
    if days < 1 {
        return Err(anyhow!(
            "Days can be set at minimum to 1, but are set to {}.",
            src
        ));
    }
    Ok(days)
}

#[derive(Parser, Debug, Clone)]
#[command(version, about, author, long_about = None)]
pub struct Cli {
    /// Netlify Site Id
    #[arg(long)]
    site_id: String,

    /// Netlify token
    #[arg(long)]
    token: String,

    /// Number of days in the past from today to collect analytics data for
    //#[clap(long, default_value_t = 30)]
    #[clap(long, default_value_t = 30, value_parser = parse_days)]
    days: i64,
}

#[derive(Debug, Clone)]
enum Metric {
    Pageviews,
    Visitors,
    Pages,
    Bandwidth,
    NotFound,
    Sources,
}

impl Display for Metric {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Metric::Pageviews => write!(f, "pageviews"),
            Metric::Visitors => write!(f, "visitors"),
            Metric::Pages => write!(f, "pages"),
            Metric::Bandwidth => write!(f, "bandwidth"),
            Metric::NotFound => write!(f, "not_found"),
            Metric::Sources => write!(f, "sources"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct MetricRange {
    start: String,
    end: String,
    timezone: String,
}

impl MetricRange {
    fn new(days: &i64) -> Self {
        let start = Local::today().and_hms_milli(0, 0, 0, 0) - Duration::days(*days);
        let end = Local::today().and_hms_milli(23, 59, 59, 999);
        MetricRange {
            start: start.timestamp_millis().to_string(),
            end: end.timestamp_millis().to_string(),
            timezone: format!("{}", start.offset()).replace(":", ""),
        }
    }
}

fn get_metrics_url_for<'a>(
    args: &'a Cli,
    range: &'a MetricRange,
    metric: &'a Metric,
) -> (&'a Metric, String) {
    (
        metric,
        format!("https://analytics.services.netlify.com/v2/{site_id}/{metric}?from={start}&to={end}&timezone={timezone}&resolution=day",
            site_id = args.site_id,
            metric = metric,
            start = range.start,
            end = range.end,
            timezone = range.timezone,
        ).to_string(),
    )
}

async fn get_metrics(client: &Client, token: &str, url: &str) -> Result<String> {
    client
        .get(url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .header("Pragma", "no-cache")
        .header("Cache-Control", "no-cache")
        .send()
        .await?
        .text()
        .await
        .with_context(|| format!("Failed getting a response for {}", url))
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TupleResult {
    pub data: Vec<(u64, u64)>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PathItemResult {
    pub path: String,
    pub count: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PathResult {
    pub data: Vec<PathItemResult>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BandwidthDataItemResult {
    pub start: u64,
    pub end: u64,
    pub site_bandwidth: u64,
    pub account_bandwidth: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BandwidthResult {
    pub data: Vec<BandwidthDataItemResult>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MetricsResult {
    pub pageviews: Option<TupleResult>,
    pub visitors: Option<TupleResult>,
    pub pages: Option<PathResult>,
    pub bandwidth: Option<BandwidthResult>,
    pub not_found: Option<PathResult>,
    pub sources: Option<PathResult>,
}

impl MetricsResult {
    fn new() -> Self {
        Self {
            pageviews: None,
            visitors: None,
            pages: None,
            bandwidth: None,
            not_found: None,
            sources: None,
        }
    }
    fn update(&mut self, metric: &Metric, text: String) {
        match metric {
            Metric::Pageviews => self.pageviews = from_str(&text).ok(),
            Metric::Visitors => self.visitors = from_str(&text).ok(),
            Metric::Pages => self.pages = from_str(&text).ok(),
            Metric::Bandwidth => self.bandwidth = from_str(&text).ok(),
            Metric::NotFound => self.not_found = from_str(&text).ok(),
            Metric::Sources => self.sources = from_str(&text).ok(),
        };
    }
}

pub async fn run(args: &Cli) -> Result<()> {
    let range = MetricRange::new(&args.days);
    info!("MetricRange: {:?}", range);

    let urls = vec![
        get_metrics_url_for(&args, &range, &Metric::Pageviews),
        get_metrics_url_for(&args, &range, &Metric::Visitors),
        get_metrics_url_for(&args, &range, &Metric::Pages),
        get_metrics_url_for(&args, &range, &Metric::Bandwidth),
        get_metrics_url_for(&args, &range, &Metric::NotFound),
        get_metrics_url_for(&args, &range, &Metric::Sources),
    ];

    info!("Started to fetch metrics");
    let client = Client::new();
    let metrics = stream_iter(urls)
        .map(|(metric, url)| {
            let client = &client;
            let token = args.token.clone();
            async move { (metric, get_metrics(client, &token, &url).await) }
        })
        .buffer_unordered(100);
    info!("Metrics fetched!");

    let result = metrics
        .fold(
            MetricsResult::new(),
            |mut acc, (metric, result)| async move {
                match result {
                    Ok(text) => acc.update(metric, text),
                    Err(e) => error!("Got an error for {} metric: {}", metric, e),
                };
                acc
            },
        )
        .await;

    println!("{}", to_string_pretty(&result)?);

    Ok(())
}
