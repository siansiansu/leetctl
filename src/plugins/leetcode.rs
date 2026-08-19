use self::req::{Json, Mode, Req};
use crate::{
    Result,
    config::{self, Config},
};
use std::{collections::HashMap, str::FromStr, time::Duration};
use wreq::{
    Client, Response,
    header::{HeaderMap, HeaderName, HeaderValue},
};
use wreq_util::Emulation;

/// LeetCode API set
#[derive(Clone)]
pub struct LeetCode {
    pub conf: Config,
    client: Client,
}

impl LeetCode {
    /// Parse reqwest headers
    fn headers(mut headers: HeaderMap, ts: Vec<(&str, &str)>) -> Result<HeaderMap> {
        for (k, v) in ts.into_iter() {
            let name = HeaderName::from_str(k)?;
            let value = HeaderValue::from_str(v)?;
            headers.insert(name, value);
        }

        Ok(headers)
    }

    /// New LeetCode client
    pub fn new() -> Result<LeetCode> {
        let conf = config::Config::locate()?;
        let (cookie, csrf) = if conf.cookies.csrf.is_empty() || conf.cookies.session.is_empty() {
            let cookies = super::chrome::cookies()?;
            (cookies.to_string(), cookies.csrf)
        } else {
            (conf.cookies.clone().to_string(), conf.cookies.clone().csrf)
        };
        let default_headers = LeetCode::headers(
            HeaderMap::new(),
            vec![
                ("Cookie", &cookie),
                ("x-csrftoken", &csrf),
                ("x-requested-with", "XMLHttpRequest"),
            ],
        )?;

        // The emulation profile supplies the TLS and HTTP/2 fingerprint plus the header set a
        // real Chrome sends; the user agent is then pinned to the Chrome actually installed,
        // because Cloudflare ties `cf_clearance` to the agent string that earned it.
        let client = Client::builder()
            .emulation(Emulation::Chrome137)
            .user_agent(super::chrome::user_agent())
            .default_headers(default_headers)
            .gzip(true)
            .connect_timeout(Duration::from_secs(30))
            // Without a ceiling on the whole exchange a stalled response hangs the command
            // forever: `poll_verify` bounds how long it keeps polling, not how long any one
            // request may take. Generous enough for the full problem-catalogue download.
            .timeout(Duration::from_secs(60))
            .build()?;

        Ok(LeetCode { conf, client })
    }

    /// Get category problems
    pub async fn get_category_problems(self, category: &str) -> Result<Response> {
        trace!("Requesting {} problems...", category);
        let url = &self.conf.sys.urls.problems(category);

        Req {
            refer: None,
            info: false,
            json: None,
            mode: Mode::Get,
            name: "get_category_problems",
            url: url.to_string(),
        }
        .send(&self.client)
        .await
    }

    pub async fn get_question_ids_by_tag(self, slug: &str) -> Result<Response> {
        trace!("Requesting {} ref problems...", slug);
        let url = &self.conf.sys.urls.graphql;
        let mut json: Json = HashMap::new();
        json.insert("operationName", "getTopicTag".to_string());
        json.insert("variables", r#"{"slug": "$slug"}"#.replace("$slug", slug));
        json.insert(
            "query",
            [
                "query getTopicTag($slug: String!) {",
                "  topicTag(slug: $slug) {",
                "    questions {",
                "      questionId",
                "    }",
                "  }",
                "}",
            ]
            .join("\n"),
        );

        Req {
            refer: Some(self.conf.sys.urls.tag(slug)),
            info: false,
            json: Some(json),
            mode: Mode::Post,
            name: "get_question_ids_by_tag",
            url: (*url).to_string(),
        }
        .send(&self.client)
        .await
    }

    pub async fn get_user_info(self) -> Result<Response> {
        trace!("Requesting user info...");
        let url = &self.conf.sys.urls.graphql;
        let mut json: Json = HashMap::new();
        json.insert("operationName", "a".to_string());
        json.insert(
            "query",
            "query a {
                 user {
                     username
                     isCurrentUserPremium
                 }
             }"
            .to_owned(),
        );

        Req {
            refer: None,
            info: false,
            json: Some(json),
            mode: Mode::Post,
            name: "get_user_info",
            url: (*url).to_string(),
        }
        .send(&self.client)
        .await
    }

    /// Get daily problem
    pub async fn get_question_daily(self) -> Result<Response> {
        trace!("Requesting daily problem...");
        let url = &self.conf.sys.urls.graphql;
        let mut json: Json = HashMap::new();

        match self.conf.cookies.site {
            config::LeetcodeSite::LeetcodeCom => {
                json.insert("operationName", "daily".to_string());
                json.insert(
                    "query",
                    [
                        "query daily {",
                        "  activeDailyCodingChallengeQuestion {",
                        "    question {",
                        "      questionFrontendId",
                        "    }",
                        "  }",
                        "}",
                    ]
                    .join("\n"),
                );
            }
            config::LeetcodeSite::LeetcodeCn => {
                json.insert("operationName", "questionOfToday".to_string());
                json.insert(
                    "query",
                    [
                        "query questionOfToday {",
                        "  todayRecord {",
                        "    question {",
                        "      questionFrontendId",
                        "    }",
                        "  }",
                        "}",
                    ]
                    .join("\n"),
                );
            }
        }

        Req {
            refer: None,
            info: false,
            json: Some(json),
            mode: Mode::Post,
            name: "get_question_daily",
            url: (*url).to_string(),
        }
        .send(&self.client)
        .await
    }

    /// Get specific problem detail
    pub async fn get_question_detail(self, slug: &str) -> Result<Response> {
        trace!("Requesting {} detail...", slug);
        let refer = self.conf.sys.urls.problem(slug);
        let mut json: Json = HashMap::new();
        json.insert(
            "query",
            [
                "query getQuestionDetail($titleSlug: String!) {",
                "  question(titleSlug: $titleSlug) {",
                "    content",
                "    stats",
                "    codeDefinition",
                "    sampleTestCase",
                "    exampleTestcases",
                "    enableRunCode",
                "    metaData",
                "    translatedContent",
                "  }",
                "}",
            ]
            .join("\n"),
        );

        json.insert(
            "variables",
            r#"{"titleSlug": "$titleSlug"}"#.replace("$titleSlug", slug),
        );

        json.insert("operationName", "getQuestionDetail".to_string());

        Req {
            refer: Some(refer),
            info: false,
            json: Some(json),
            mode: Mode::Post,
            name: "get_problem_detail",
            url: self.conf.sys.urls.graphql,
        }
        .send(&self.client)
        .await
    }

    /// Send code to judge
    pub async fn run_code(self, j: Json, url: String, refer: String) -> Result<Response> {
        info!("Sending code to judge...");
        Req {
            refer: Some(refer),
            info: false,
            json: Some(j),
            mode: Mode::Post,
            name: "run_code",
            url,
        }
        .send(&self.client)
        .await
    }

    /// Get the result of submission / testing
    pub async fn verify_result(self, id: String) -> Result<Response> {
        trace!("Verifying result...");
        let url = self.conf.sys.urls.verify(&id);

        Req {
            refer: None,
            info: false,
            json: None,
            mode: Mode::Get,
            name: "verify_result",
            url,
        }
        .send(&self.client)
        .await
    }
}

/// Sub-module for leetcode, simplify requests
mod req {
    use crate::err::Error;
    use std::collections::HashMap;
    use wreq::{Client, Response, Uri};

    /// Standardize json format
    pub type Json = HashMap<&'static str, String>;

    /// Standardize request mode
    pub enum Mode {
        Get,
        Post,
    }

    /// LeetCode request prototype
    pub struct Req {
        pub refer: Option<String>,
        pub json: Option<Json>,
        pub info: bool,
        pub mode: Mode,
        pub name: &'static str,
        pub url: String,
    }

    /// The `Origin` a browser would send for a request to `url`: its scheme and authority.
    fn origin_of(url: &str) -> Result<String, Error> {
        let uri: Uri = url
            .parse()
            .map_err(|e| anyhow::anyhow!("parse {url}: {e}"))?;
        match (uri.scheme_str(), uri.authority()) {
            (Some(scheme), Some(authority)) => Ok(format!("{scheme}://{authority}")),
            _ => Err(anyhow::anyhow!("{url} has no origin").into()),
        }
    }

    impl Req {
        pub async fn send(self, client: &Client) -> Result<Response, Error> {
            trace!("Running leetcode::{}...", self.name);
            if self.info {
                info!("{}", self.name);
            }
            let url = self.url.to_owned();
            let referer = self.refer.unwrap_or(url);

            // Browsers attach `Origin` to POSTs, not to plain GETs. Sending it on a GET is one
            // more way the request reads as scripted to Cloudflare.
            let req = match self.mode {
                Mode::Get => client.get(&self.url),
                Mode::Post => client
                    .post(&self.url)
                    .header("Origin", origin_of(&self.url)?)
                    .json(&self.json),
            };

            Ok(req.header("Referer", referer).send().await?)
        }
    }
}
