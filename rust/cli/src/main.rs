use dotenv::dotenv;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, USER_AGENT};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct PullRequest {}

#[derive(Debug, Serialize, Deserialize)]
struct Issue {
    number: usize,
    title: String,
    pull_request: Option<PullRequest>,
}

fn construct_new_url(headers: &HeaderMap) -> Option<String> {
    headers.get("link").and_then(|link_header| {
        link_header.to_str().ok().and_then(|link_value| {
            link_value.contains("rel=\"\"").then(|| {
                link_value
                    .split(";")
                    .collect::<Vec<&str>>()
                    .get(0)
                    .expect("Could not find new url with page")
                    .to_string()
            })
        })
    })
}

fn get_issues_wrapper() {}

async fn get_issues() -> Vec<Issue> {
    let token =
        std::env::var("GITHUB_PAT").expect("Expected GITHUB_PAT in env file");
    let request_url = format!(
        "https://api.github.com/repos/{owner}/{repo}/issues?state=open&page=1&per_page=100",
        owner = "zed-industries",
        repo = "zed",
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&request_url)
        .header(
            AUTHORIZATION,
            format!("Bearer {token}", token = token),
        )
        .header(USER_AGENT, "rust web-api")
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await;

    let response = match response {
        Ok(res) if res.status().is_success() => res,
        _ => return Vec::new(),
    };

    let new_url = construct_new_url(response.headers());

    let issues = response
        .json::<Vec<Issue>>()
        .await
        .expect("Something wen wrong while parsing")
        .into_iter()
        .filter(|issue| issue.pull_request.is_none())
        .collect::<Vec<_>>();

    if let Some(new_url) = new_url {
        let more_issues = get_issues_wrapper(Some(new_url)).await;
        return issues.iter().chain(more_issues).collect();
    }

    issues
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let issues = get_issues().await;

    println!("{:?}", issues)

    // for issue in issues {
    //     let reactions = get_issue_reactions(issue);
    // }
}
