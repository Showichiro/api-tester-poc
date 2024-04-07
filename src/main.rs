use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Parser)]
struct Cli {
    case_dir: std::path::PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]

enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
}

#[derive(Serialize, Deserialize, Debug)]
struct RequestJson {
    url: String,
    method: HttpMethod,
    query: Option<HashMap<String, String>>,
    body: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ExpectedJson {
    status: u16,
    json: Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct OutPutJson {
    status: u16,
    json: Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct ResultJsonRow {
    status_diff: bool,
    json_diff: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let dir_entries = fs::read_dir(&args.case_dir)?;
    let mut results: HashMap<String, ResultJsonRow> = HashMap::new();
    for entry in dir_entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let input_path = entry.path().join("input.json");
            let mut input_file = File::open(input_path).await?;
            let mut input_data = String::new();
            input_file.read_to_string(&mut input_data).await?;
            // read request file
            let request_json: RequestJson = serde_json::from_str(input_data.as_str())?;
            let url = request_json.url;
            // call api
            let mut query = vec![];
            for (key, value) in request_json.query.unwrap_or_default() {
                query.push((key, value));
            }
            let method = request_json.method;
            let body = request_json.body;
            let client = reqwest::Client::new();
            let response = match method {
                HttpMethod::GET => client.get(url).query(&query).send().await?,
                HttpMethod::POST => {
                    client
                        .post(url)
                        .query(&query)
                        .json(&body.unwrap_or_default())
                        .send()
                        .await?
                }
                HttpMethod::PUT => {
                    client
                        .put(url)
                        .query(&query)
                        .json(&body.unwrap_or_default())
                        .send()
                        .await?
                }
                HttpMethod::DELETE => {
                    client
                        .delete(url)
                        .query(&query)
                        .json(&body.unwrap_or_default())
                        .send()
                        .await?
                }
                HttpMethod::PATCH => todo!(),
            };
            let status = response.status();
            let response_data: Value = response.json().await?;
            // write response file
            let output_path = entry.path().join("output.json");
            let mut output_file = File::create(output_path).await?;
            let output_json = OutPutJson {
                status: status.as_u16(),
                json: response_data.clone(),
            };
            output_file
                .write_all(serde_json::to_string(&output_json)?.as_bytes())
                .await?;
            let expected_output_path = entry.path().join("expected.json");
            let mut expected_file = File::open(expected_output_path).await?;
            let mut expected_data = String::new();
            expected_file.read_to_string(&mut expected_data).await?;
            let expected_json: ExpectedJson = serde_json::from_str(expected_data.as_str())?;
            // assert expected and actual
            let diff = serde_json_diff::values(response_data, expected_json.json);
            let result = ResultJsonRow {
                status_diff: status.as_u16() != expected_json.status,
                json_diff: diff.is_some(),
            };
            let case_name = entry.file_name().into_string().unwrap();
            results.insert(case_name, result);
        }
    }
    // write result file
    let result_file_path = args.case_dir.join("result.json");
    let mut result_file = File::create(result_file_path).await?;
    let result_json = serde_json::to_string(&results)?;
    result_file.write_all(result_json.as_bytes()).await?;
    Ok(())
}
