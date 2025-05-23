use std::collections::HashMap;

use dotenv::dotenv;
use futures::{StreamExt, future::BoxFuture, stream::FuturesUnordered};
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

#[derive(Debug, Serialize, Deserialize)]
struct User {
    login: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct IssueReaction {
    content: String,
    user: User,
}

struct GithubApi {
    client: reqwest::Client,
    token: String,
    user_agent: String,
    owner: String,
    repository: String,
}

impl GithubApi {
    fn new(
        token: String,
        user_agent: String,
        owner: String,
        repository: String,
    ) -> Self {
        let client = reqwest::Client::new();
        GithubApi {
            client,
            token,
            user_agent,
            owner,
            repository,
        }
    }

    fn construct_new_url(
        &self,
        headers: &HeaderMap,
    ) -> Option<String> {
        headers
            .get("link")
            .and_then(|link_header| {
                link_header
                    .to_str()
                    .ok()
                    .and_then(|link_value| {
                        link_value.split(',').find_map(|link| {
                            if link.contains("rel=\"next\"") {
                                link.split(';').next().map(|url| {
                                    url.trim()
                                        .trim_start_matches('<')
                                        .trim_end_matches('>')
                                        .to_string()
                                })
                            } else {
                                None
                            }
                        })
                    })
            })
    }

    fn get_issues_wrapper<'a>(
        &'a self,
        url: Option<String>,
    ) -> BoxFuture<'a, Vec<Issue>> {
        Box::pin(self.get_issues(url))
    }

    async fn get_issues(
        &self,
        url: Option<String>,
    ) -> Vec<Issue> {
        let request_url = url.unwrap_or(format!(
            "https://api.github.com/repos/{owner}/{repo}/issues?state=open&page=1&per_page=100",
            owner = self.owner,
            repo = self.repository,
        ));

        let response = self
            .client
            .get(&request_url)
            .header(
                AUTHORIZATION,
                format!("Bearer {token}", token = self.token),
            )
            .header(USER_AGENT, &self.user_agent)
            .header(ACCEPT, "application/vnd.github+json")
            .send()
            .await;

        let response = match response {
            Ok(res) if res.status().is_success() => res,
            _ => return Vec::new(),
        };

        let new_url = self.construct_new_url(response.headers());

        let issues = response
            .json::<Vec<Issue>>()
            .await
            .expect("Something wen wrong while parsing")
            .into_iter()
            .filter(|issue| issue.pull_request.is_none())
            .collect::<Vec<_>>();

        if let Some(new_url) = new_url {
            let more_issues = self
                .get_issues_wrapper(Some(new_url))
                .await;
            return issues
                .into_iter()
                .chain(more_issues)
                .collect();
        }

        issues
    }

    async fn get_issue_reactions(
        &self,
        issue_id: usize,
    ) -> Vec<IssueReaction> {
        let request_url = format!(
            "https://api.github.com/repos/{owner}/{repo}/issues/{issue_id}/reactions",
            owner = self.owner,
            repo = self.repository,
        );

        let response = self
            .client
            .get(&request_url)
            .header(
                AUTHORIZATION,
                format!("Bearer {token}", token = self.token),
            )
            .header(USER_AGENT, &self.user_agent)
            .header(ACCEPT, "application/vnd.github+json")
            .send()
            .await
            .expect("Something went wrong while fetching");

        let resolved_response = response
            .json::<Vec<IssueReaction>>()
            .await
            .expect("Something went wrong while parsing");
        resolved_response
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let user_agent =
        std::env::var("USER_AGENT").expect("Expected USER_AGENT in env file");
    let token = std::env::var("GITHUB_TOKEN")
        .expect("Expected GITHUB_TOKEN in env file");

    // cargo run -- nikitabobko AeroSpace 30
    let owner = std::env::args()
        .nth(1)
        .expect("Owner name is required");
    let repo = std::env::args()
        .nth(2)
        .expect("Repository name is required");
    let limit = std::env::args()
        .nth(3)
        .expect("Limit is required")
        .parse::<usize>()
        .expect("Limit must be a number");

    let github_api = GithubApi::new(token, user_agent, owner, repo);

    let issues = github_api.get_issues(None).await;
    let mut futures = FuturesUnordered::new();

    for issue in &issues {
        let issue_number = issue.number;
        let github_api_ref = &github_api;
        futures.push({
            async move {
                let reactiions = github_api_ref
                    .get_issue_reactions(issue_number)
                    .await;
                (issue_number, reactiions)
            }
        });
    }

    let mut results: HashMap<usize, usize> = HashMap::new();

    while let Some((issue_number, reactions)) = futures.next().await {
        let reaction_count = reactions
            .iter()
            .filter(|r| r.content == "+1")
            .count();
        if reaction_count > 0 {
            results
                .entry(issue_number)
                .and_modify(|e| *e += reaction_count)
                .or_insert(reaction_count);
        }
    }

    let mut sorted_results: Vec<_> = results.into_iter().collect();
    sorted_results.sort_by(|a, b| b.1.cmp(&a.1));

    let now = chrono::Utc::now();
    println!(
        "*Updated on {} (UTC)*\n",
        now.format("%d-%m-%Y %H:%M:%S"),
    );

    for (index, (issue_number, upvotes)) in sorted_results.iter().enumerate() {
        if index >= limit {
            break;
        }

        println!(
            "{}. {}# ({} 👍)",
            index + 1,
            issue_number,
            upvotes,
        )
    }

    println!("Amount of issue: {:?}", issues.len());
}
