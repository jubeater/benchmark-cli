mod analysis;
mod input;
mod model;

use analysis::{AnalysisResult, analyze};
use input::{parse_lines, parse_pricing_model};

fn main() {
    let price_table = parse_pricing_model("./data/pricing.json");
    match price_table {
        Ok(table) => {
            let requests = parse_lines("./data/sample.jsonl");
            match requests {
                Ok(result) => {
                    let analysis_result = analyze(&result, &table);
                    print!("done!");
                }
                Err(e) => print!("io err happened!{:?}", e),
            }
        }
        Err(e) => {
            print!("pricing load err!{:?}", e);
        }
    }
}
