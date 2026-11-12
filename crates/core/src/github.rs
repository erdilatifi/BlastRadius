use anyhow::{Context, Result};
use reqwest::{Client, Method, RequestBuilder};
use serde_json::{Value, json};

pub enum Change {
    Write(String),
    Delete,
}

pub struct GitHub {
    http: Client,
    api: String,
    repo: String,
    token: String,
}

impl GitHub {
    pub fn new(api: &str, repo: &str, token: &str) -> Result<GitHub> {
        let http = Client::builder().user_agent("brctl").build()?;
        Ok(GitHub {
            http,
            api: api.trim_end_matches('/').into(),
            repo: repo.into(),
            token: token.into(),
        })
    }

    fn req(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}/repos/{}/{path}", self.api, self.repo);
        self.http
            .request(method, url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
    }

    async fn send(&self, req: RequestBuilder) -> Result<Value> {
        let res = req.send().await?;
        let status = res.status();
        let body: Value = res.json().await.unwrap_or(Value::Null);
        anyhow::ensure!(
            status.is_success(),
            "GitHub answered {status}: {}",
            body["message"].as_str().unwrap_or("")
        );
        Ok(body)
    }

    pub async fn open_pr(
        &self,
        branch: &str,
        path: &str,
        change: Change,
        title: &str,
        body: &str,
    ) -> Result<String> {
        let main = self
            .send(self.req(Method::GET, "git/ref/heads/main"))
            .await?;
        let sha = main["object"]["sha"].as_str().context("main has no sha")?;
        let refs = json!({"ref": format!("refs/heads/{branch}"), "sha": sha});
        self.send(self.req(Method::POST, "git/refs").json(&refs))
            .await?;
        let file = format!("contents/{path}");
        match change {
            Change::Write(text) => {
                let put =
                    json!({"message": title, "branch": branch, "content": base64(text.as_bytes())});
                self.send(self.req(Method::PUT, &file).json(&put)).await?;
            }
            Change::Delete => {
                let current = self
                    .send(self.req(Method::GET, &format!("{file}?ref={branch}")))
                    .await?;
                let sha = current["sha"].as_str().context("file has no sha")?;
                let del = json!({"message": title, "branch": branch, "sha": sha});
                self.send(self.req(Method::DELETE, &file).json(&del))
                    .await?;
            }
        }
        let pr = json!({"title": title, "head": branch, "base": "main", "body": body});
        let pr = self.send(self.req(Method::POST, "pulls").json(&pr)).await?;
        Ok(pr["html_url"]
            .as_str()
            .context("pull request has no url")?
            .into())
    }
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            let c = if i <= chunk.len() {
                ABC[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            };
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn server() -> MockServer {
        let s = MockServer::start().await;
        let ok = |body: Value| ResponseTemplate::new(200).set_body_json(body);
        Mock::given(method("GET"))
            .and(path("/repos/o/r/git/ref/heads/main"))
            .and(header("authorization", "Bearer t"))
            .respond_with(ok(json!({"object": {"sha": "abc"}})))
            .mount(&s)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/o/r/git/refs"))
            .and(body_partial_json(
                json!({"ref": "refs/heads/b", "sha": "abc"}),
            ))
            .respond_with(ok(json!({})))
            .expect(1)
            .mount(&s)
            .await;
        Mock::given(method("POST"))
            .and(path("/repos/o/r/pulls"))
            .and(body_partial_json(
                json!({"head": "b", "base": "main", "title": "t"}),
            ))
            .respond_with(ok(json!({"html_url": "https://pr/1"})))
            .mount(&s)
            .await;
        s
    }

    #[tokio::test]
    async fn opens_pr_writing_a_file() {
        let s = server().await;
        Mock::given(method("PUT"))
            .and(path("/repos/o/r/contents/tenants/x/values.yaml"))
            .and(body_partial_json(
                json!({"branch": "b", "content": "bmFtZTogeAo="}),
            ))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
            .expect(1)
            .mount(&s)
            .await;
        let gh = GitHub::new(&s.uri(), "o/r", "t").unwrap();
        let url = gh
            .open_pr(
                "b",
                "tenants/x/values.yaml",
                Change::Write("name: x\n".into()),
                "t",
                "",
            )
            .await
            .unwrap();
        assert_eq!(url, "https://pr/1");
    }

    #[tokio::test]
    async fn opens_pr_deleting_a_file() {
        let s = server().await;
        Mock::given(method("GET"))
            .and(path("/repos/o/r/contents/f"))
            .and(query_param("ref", "b"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"sha": "f1"})))
            .mount(&s)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/repos/o/r/contents/f"))
            .and(body_partial_json(json!({"sha": "f1", "branch": "b"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&s)
            .await;
        let gh = GitHub::new(&s.uri(), "o/r", "t").unwrap();
        assert_eq!(
            gh.open_pr("b", "f", Change::Delete, "t", "").await.unwrap(),
            "https://pr/1"
        );
    }

    #[tokio::test]
    async fn reports_github_errors() {
        let s = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(401).set_body_json(json!({"message": "Bad credentials"})),
            )
            .mount(&s)
            .await;
        let gh = GitHub::new(&s.uri(), "o/r", "t").unwrap();
        let e = gh
            .open_pr("b", "f", Change::Delete, "t", "")
            .await
            .unwrap_err();
        assert_eq!(
            e.to_string(),
            "GitHub answered 401 Unauthorized: Bad credentials"
        );
    }

    #[test]
    fn encodes_base64() {
        for (raw, enc) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
        ] {
            assert_eq!(base64(raw.as_bytes()), enc);
        }
    }
}
