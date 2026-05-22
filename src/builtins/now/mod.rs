use anyhow::Result;
use chrono::{Datelike, Local, Timelike};
use serde::Serialize;

use crate::cli::CommandContext;
use crate::runtime::output::{self, Envelope, PrintMode};

#[derive(Debug, Serialize)]
struct NowData {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    timezone: String,
}

pub fn run(ctx: &CommandContext) -> Result<u8> {
    let now = Local::now();
    let naive = now.naive_local();
    let offset = now.offset();

    match ctx.print {
        PrintMode::Json => {
            let data = NowData {
                year: naive.year() as u16,
                month: naive.month() as u8,
                day: naive.day() as u8,
                hour: naive.hour() as u8,
                minute: naive.minute() as u8,
                second: naive.second() as u8,
                timezone: offset.to_string(),
            };
            let payload = Envelope {
                ok: true,
                command: "now",
                data,
                warnings: vec![],
                meta: serde_json::json!({}),
            };
            output::print_json(&payload)?;
        }
        _ => {
            // YYYY-MM-DD HH:MM:SS (timezone)
            println!("{}", now.format("%Y-%m-%d %H:%M:%S (%:z)"));
        }
    }

    Ok(0)
}
