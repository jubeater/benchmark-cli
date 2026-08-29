mod analysis;
mod input;
mod model;
mod report;

use analysis::analyze;
use input::{parse_lines, parse_pricing_model};
use report::render;

fn main() {
    let price_table = parse_pricing_model("./data/pricing.json");
    match price_table {
        Ok(table) => {
            let requests = parse_lines("./data/sample.jsonl");
            match requests {
                Ok(result) => {
                    let analysis_result = analyze(&result, &table);
                    let output_string = render(&analysis_result);
                    print!("{}", output_string);
                }
                Err(e) => print!("io err happened!{:?}", e),
            }
        }
        Err(e) => {
            print!("pricing load err!{:?}", e);
        }
    }
}
